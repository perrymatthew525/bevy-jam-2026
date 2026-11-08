use std::f32::consts::TAU;
use std::time::Duration;

use bevy::asset::{AssetMetaCheck, RenderAssetUsages};
use bevy::audio::Volume;
use bevy::math::ops::floor;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::time::common_conditions::on_timer;
use bevy::ui_widgets::{Activate, ActivateOnPress, Button};

const ARENA_HALF_WIDTH: f32 = 500.0;
const WALL_THICKNESS: f32 = 30.0;
const WALL_LENGTH: f32 = ARENA_HALF_WIDTH * 2.0;
const WALL_SIZE_LONG: Vec2 = Vec2::new(WALL_LENGTH + WALL_THICKNESS, WALL_THICKNESS);
const WALL_SIZE_TALL: Vec2 = Vec2::new(WALL_THICKNESS, WALL_LENGTH);
const WALL_Z: f32 = 2.0;
const HERO_SPEED: f32 = 100.0;
const HERO_WIDTH: f32 = 14.0;
const HERO_HEIGHT: f32 = 19.0;
const HERO_Z: f32 = 1.0;
const ENEMY_SPEED: f32 = 50.0;
const ENEMY_WIDTH: f32 = 14.0;
const ENEMY_HEIGHT: f32 = 16.0;
const ENEMY_DAMAGE: u32 = 10;
const ENEMY_Z: f32 = 1.0;
const FLOOR_Z: f32 = 0.0;
const PAINT_CELL_SIZE: f32 = 2.0;
const SPLAT_RADIUS: i32 = 10;
const BULLET_WIDTH: f32 = 5.0;
const BULLET_SIZE: Vec2 = Vec2::new(BULLET_WIDTH, BULLET_WIDTH);
const BULLET_SPEED: f32 = 300.0;
const BULLET_Z: f32 = 1.0;
const NUM_BULLETS_SPAWN: i32 = 3;
const KNOCKBACK: f32 = 20.0;
const HP_FONT_SIZE: f32 = 20.0;
const TEXT_COLOR_RED: Color = Color::srgb_u8(191, 9, 47);
const HP_TEXT_PADDING: f32 = 10.0;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Paint {
    None,
    Lead,
    Iron,
    Silver,
    Gold,
}

impl Paint {
    fn sprite_color(self) -> Color {
        match self {
            Self::None => Srgba::hex("#00050e").unwrap().into(),
            Self::Lead => Srgba::hex("#3798dd").unwrap().into(),
            Self::Iron => Srgba::hex("#ed866c").unwrap().into(),
            Self::Silver => Srgba::hex("#fafafa").unwrap().into(),
            Self::Gold => Srgba::hex("#fff0bf").unwrap().into(),
        }
    }

    fn ground_color(self) -> Color {
        match self {
            Self::None => Srgba::hex("#132440").unwrap().into(),
            Self::Lead => Srgba::hex("#16476A").unwrap().into(),
            Self::Iron => Srgba::hex("#b74326").unwrap().into(),
            Self::Silver => Srgba::hex("#9dc7e5").unwrap().into(),
            Self::Gold => Srgba::hex("#eeca53").unwrap().into(),
        }
    }

    fn next_color(self) -> Paint {
        match self {
            Self::None => Paint::Lead,
            Self::Lead => Paint::Iron,
            Self::Iron => Paint::Silver,
            Self::Silver => Paint::Gold,
            Self::Gold => Paint::None,
        }
    }
}

#[derive(Component)]
struct Hero {
    paint: Paint,
    hitpoints: u32,
}

#[derive(Component)]
struct Enemy {
    paint: Paint,
}

#[derive(Component)]
struct HpUi;

#[derive(Component)]
struct LeadUi;

#[derive(Component)]
struct IronUi;

#[derive(Component)]
struct SilverUi;

#[derive(Component)]
struct GoldUi;

#[derive(Component)]
struct Wall;

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component)]
struct Bullet {
    paint: Paint,
}

#[derive(Component)]
struct Collider {
    half_size: Vec2,
}

#[derive(Resource)]
struct EnemyLayout(Handle<TextureAtlasLayout>);

#[derive(Resource)]
struct FloorImage(Handle<Image>);

#[derive(Resource)]
struct PaintGrid {
    width: usize,
    height: usize,
    cells: Vec<Paint>,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Menu,
    Playing,
    GameOver,
}

#[derive(Deref, DerefMut, Resource)]
struct SpawnTimer(Timer);

impl Default for SpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2.0, TimerMode::Repeating))
    }
}

#[derive(Deref, DerefMut, Resource)]
struct MetaSpawnTimer(Timer);

impl Default for MetaSpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(30.0, TimerMode::Repeating))
    }
}

#[derive(Deref, DerefMut, Resource)]
struct HurtTimer(Timer);

impl Default for HurtTimer {
    fn default() -> Self {
        let mut timer = Timer::from_seconds(0.15, TimerMode::Once);
        timer.finish();
        Self(timer)
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Wasm builds will check for meta files (that don't exist) if this isn't set.
                    // This causes errors and even panics in web builds on itch.
                    // See https://github.com/bevyengine/bevy_github_ci_template/issues/48.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                // Default to nearest neighbor rendering
                .set(ImagePlugin::default_nearest()),
        )
        .init_state::<AppState>()
        .add_systems(OnEnter(AppState::Menu), setup_menu)
        .add_systems(OnEnter(AppState::GameOver), setup_game_over)
        .insert_resource(ClearColor(Paint::None.sprite_color()))
        .add_systems(Startup, setup)
        .add_observer(on_activate_start_game.run_if(in_state(AppState::Menu)))
        .add_observer(on_activate_start_game.run_if(in_state(AppState::GameOver)))
        .add_systems(OnEnter(AppState::Playing), setup_game)
        .add_systems(
            Update,
            (hero_movement, wall_collision, update_camera)
                .chain()
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(Update, enemy_spawn.run_if(in_state(AppState::Playing)))
        .add_systems(
            Update,
            (enemy_movement, enemy_collision, enemy_hero_collision)
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(
            Update,
            animate_sprites
                .run_if(on_timer(Duration::from_millis(200)))
                .run_if(in_state(AppState::Playing)),
        )
        // .add_systems(Update, draw_paint.run_if(in_state(AppState::Playing)))
        .add_systems(Update, recolor_sprite.run_if(in_state(AppState::Playing)))
        .add_systems(
            Update,
            fire_bullets
                .run_if(on_timer(Duration::from_millis(300)))
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(
            Update,
            (bullet_despawn, move_bullets, bullet_collision).run_if(in_state(AppState::Playing)),
        )
        .add_systems(Update, hp_ui.run_if(in_state(AppState::Playing)))
        .add_systems(
            Update,
            arena_state_ui
                .run_if(on_timer(Duration::from_millis(200)))
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(Update, check_game_over.run_if(in_state(AppState::Playing)))
        .add_systems(Update, tick_hurt_timer.run_if(in_state(AppState::Playing)))
        .run();
}

fn setup_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Paint::None.ground_color()),
        children![(
            Button,
            ActivateOnPress,
            Hovered::default(),
            Node {
                width: px(150),
                height: px(65),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Paint::None.sprite_color()),
            children![(
                Text::new("Play"),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
            )]
        )],
        DespawnOnExit(AppState::Menu),
    ));
}

fn setup_game_over(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    paint_grid: Res<PaintGrid>,
) {
    let mut high_score_message = String::new();
    let mut lead_count = 0.0;
    let mut iron_count = 0.0;
    let mut silver_count = 0.0;
    let mut gold_count = 0.0;
    let mut total_count = 0.0;

    for i in 0..paint_grid.cells.len() {
        total_count += 1.0;
        let curr_paint = paint_grid.cells[i];
        match curr_paint {
            Paint::None => {}
            Paint::Lead => lead_count += 1.0,
            Paint::Iron => iron_count += 1.0,
            Paint::Silver => silver_count += 1.0,
            Paint::Gold => gold_count += 1.0,
        }
    }
    if gold_count > 0.0 {
        high_score_message = format!("Score: {:.2}% gold", gold_count / total_count * 100.0);
    } else if silver_count > 0.0 {
        high_score_message = format!("Score: {:.2}% silver", silver_count / total_count * 100.0);
    } else if iron_count > 0.0 {
        high_score_message = format!("Score: {:.2}% iron", iron_count / total_count * 100.0);
    } else if lead_count > 0.0 {
        high_score_message = format!("Score: {:.2}% lead", lead_count / total_count * 100.0);
    } else {
        high_score_message = format!("You died immediately!")
    }

    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: px(10.0),
            ..default()
        },
        BackgroundColor(Paint::None.ground_color()),
        children![
            (
                Text::new("Game Over"),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
                TextColor(TEXT_COLOR_RED),
            ),
            (
                Text::new(high_score_message),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
            ),
            (
                Button,
                ActivateOnPress,
                Hovered::default(),
                Node {
                    width: px(150),
                    height: px(65),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Paint::None.sprite_color()),
                children![(Text::new("Restart") /* TextFont, TextColor */,)]
            )
        ],
        DespawnOnExit(AppState::GameOver),
    ));
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 0.5,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // Music
    commands.spawn((
        AudioPlayer::new(asset_server.load("main.ogg")),
        PlaybackSettings {
            ..PlaybackSettings::LOOP
        }
        .with_volume(Volume::Linear(0.1)),
    ));

    // Enemy layout
    let enemy_layout = TextureAtlasLayout::from_grid(UVec2::new(16, 24), 4, 1, None, None);
    let enemy_layout_handle = texture_atlas_layouts.add(enemy_layout);
    commands.insert_resource(EnemyLayout(enemy_layout_handle));
}

fn setup_game(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Spawn Timer
    commands.insert_resource(SpawnTimer::default());
    commands.insert_resource(MetaSpawnTimer::default());
    commands.insert_resource(HurtTimer::default());

    // Hero
    let hero_layout = TextureAtlasLayout::from_grid(UVec2::new(16, 24), 4, 1, None, None);
    let hero_layout_handle = texture_atlas_layouts.add(hero_layout);
    commands.spawn((
        Sprite {
            color: Color::srgba(0.5, 0.8, 1.0, 1.0),
            ..Sprite::from_atlas_image(
                asset_server.load("Hero.png"),
                TextureAtlas {
                    layout: hero_layout_handle,
                    index: 0,
                },
            )
        },
        Hero {
            paint: Paint::None,
            hitpoints: 100,
        },
        Transform::from_xyz(0.0, 0.0, HERO_Z),
        Collider {
            half_size: Vec2::new(HERO_WIDTH / 2.0, HERO_HEIGHT / 2.0),
        },
        DespawnOnExit(AppState::Playing),
    ));

    // HP UI
    commands.spawn((
        Text::new("HP: "),
        TextFont {
            font_size: FontSize::Px(HP_FONT_SIZE),
            font: asset_server.load("FiraMono-Medium.ttf").into(),
            ..default()
        },
        TextColor(TEXT_COLOR_RED),
        HpUi,
        Node {
            position_type: PositionType::Absolute,
            top: px(HP_TEXT_PADDING),
            left: px(HP_TEXT_PADDING),
            ..default()
        },
        DespawnOnExit(AppState::Playing),
    ));

    // Arena state UI
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Absolute,
            bottom: px(HP_TEXT_PADDING),
            left: px(HP_TEXT_PADDING),
            row_gap: px(5.0),
            ..default()
        },
        children![
            (
                Text::new("Row 1."),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
                TextColor(Paint::Lead.sprite_color()),
                LeadUi,
            ),
            (
                Text::new("Row 2."),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
                TextColor(Paint::Iron.sprite_color()),
                IronUi,
            ),
            (
                Text::new("Row 3."),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
                TextColor(Paint::Silver.sprite_color()),
                SilverUi,
            ),
            (
                Text::new("Row 4."),
                TextFont {
                    font_size: FontSize::Px(HP_FONT_SIZE),
                    font: asset_server.load("FiraMono-Medium.ttf").into(),
                    ..default()
                },
                TextColor(Paint::Gold.sprite_color()),
                GoldUi,
            )
        ],
        DespawnOnExit(AppState::Playing),
    ));

    // Floor
    let paint_width = ((ARENA_HALF_WIDTH * 2.0) / PAINT_CELL_SIZE) as usize;
    let paint_height = ((ARENA_HALF_WIDTH * 2.0) / PAINT_CELL_SIZE) as usize;
    let paint_grid = PaintGrid {
        width: paint_width,
        height: paint_height,
        cells: vec![Paint::None; paint_width * paint_height],
    };
    let floor_image = Image::new_fill(
        Extent3d {
            width: paint_grid.width as u32,
            height: paint_grid.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &(Paint::None.ground_color().to_srgba().to_u8_array()),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let floor_image_handle = images.add(floor_image);
    commands.insert_resource(FloorImage(floor_image_handle.clone()));
    commands.insert_resource(paint_grid);
    commands.spawn((
        Sprite {
            custom_size: Some(Vec2::new(ARENA_HALF_WIDTH * 2.0, ARENA_HALF_WIDTH * 2.0)),
            ..Sprite::from_image(floor_image_handle)
        },
        Transform::from_xyz(0.0, 0.0, FLOOR_Z),
        DespawnOnExit(AppState::Playing),
    ));

    // Arena
    // Top
    commands.spawn((
        Sprite::from_color(Paint::None.sprite_color(), WALL_SIZE_LONG),
        Transform::from_translation(Vec3::new(0.0, WALL_LENGTH / 2.0, WALL_Z)),
        Collider {
            half_size: WALL_SIZE_LONG / 2.0,
        },
        Wall,
        DespawnOnExit(AppState::Playing),
    ));
    // Bottom
    commands.spawn((
        Sprite::from_color(Paint::None.sprite_color(), WALL_SIZE_LONG),
        Transform::from_translation(Vec3::new(0.0, -(WALL_LENGTH / 2.0), WALL_Z)),
        Collider {
            half_size: WALL_SIZE_LONG / 2.0,
        },
        Wall,
        DespawnOnExit(AppState::Playing),
    ));
    // Right
    commands.spawn((
        Sprite::from_color(Paint::None.sprite_color(), WALL_SIZE_TALL),
        Transform::from_translation(Vec3::new(WALL_LENGTH / 2.0, 0.0, WALL_Z)),
        Collider {
            half_size: WALL_SIZE_TALL / 2.0,
        },
        Wall,
        DespawnOnExit(AppState::Playing),
    ));
    // Left
    commands.spawn((
        Sprite::from_color(Paint::None.sprite_color(), WALL_SIZE_TALL),
        Transform::from_translation(Vec3::new(-(WALL_LENGTH / 2.0), 0.0, WALL_Z)),
        Collider {
            half_size: WALL_SIZE_TALL / 2.0,
        },
        Wall,
        DespawnOnExit(AppState::Playing),
    ));
}

fn hero_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut hero: Single<&mut Transform, With<Hero>>,
) {
    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    direction = direction.normalize_or_zero();

    hero.translation += (direction * HERO_SPEED * time.delta_secs()).extend(0.0);
}

fn wall_collision(
    mut hero: Single<(&mut Transform, &Collider), With<Hero>>,
    walls: Query<(&Transform, &Collider), (With<Wall>, Without<Hero>)>,
) {
    for (wall_transform, wall_collider) in &walls {
        let hero_bounding_rect = Aabb2d::new(hero.0.translation.truncate(), hero.1.half_size);
        let wall_bounding_rect = Aabb2d::new(
            wall_transform.translation.truncate(),
            wall_collider.half_size,
        );

        if hero_bounding_rect.intersects(&wall_bounding_rect) {
            let delta = hero_bounding_rect.center() - wall_bounding_rect.center();
            let overlap =
                hero_bounding_rect.half_size() + wall_bounding_rect.half_size() - delta.abs();
            if overlap.x < overlap.y {
                hero.0.translation.x += delta.x.signum() * overlap.x;
            } else {
                hero.0.translation.y += delta.y.signum() * overlap.y;
            }
        }
    }
}

fn enemy_hero_collision(
    mut hero: Single<(&Transform, &Collider, &mut Hero), With<Hero>>,
    mut enemy_query: Query<(&mut Transform, &Collider), (With<Enemy>, Without<Hero>)>,
    mut hurt_timer: ResMut<HurtTimer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let (hero_transform, hero_collider, hero_self) = &mut *hero;
    for (mut enemy_transform, enemy_collider) in &mut enemy_query {
        let hero_bounding_rect = Aabb2d::new(
            hero_transform.translation.truncate(),
            hero_collider.half_size,
        );
        let enemy_bounding_rect = Aabb2d::new(
            enemy_transform.translation.truncate(),
            enemy_collider.half_size,
        );

        if hero_bounding_rect.intersects(&enemy_bounding_rect) {
            let delta = enemy_bounding_rect.center() - hero_bounding_rect.center();
            let overlap =
                hero_bounding_rect.half_size() + enemy_bounding_rect.half_size() - delta.abs();
            hurt_timer.reset();
            enemy_transform.translation.x += delta.x.signum() * (overlap.x + KNOCKBACK);
            enemy_transform.translation.y += delta.y.signum() * (overlap.y + KNOCKBACK);
            hero_self.hitpoints = hero_self.hitpoints.saturating_sub(ENEMY_DAMAGE);
            commands.spawn((
                AudioPlayer::new(asset_server.load("23_orc_jump_land.wav")),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.5)),
            ));
        }
    }
}

fn enemy_spawn(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    enemy_layout: Res<EnemyLayout>,
    mut spawn_timer: ResMut<SpawnTimer>,
    mut meta_spawn_timer: ResMut<MetaSpawnTimer>,
    time: Res<Time>,
) {
    spawn_timer.tick(time.delta());
    meta_spawn_timer.tick(time.delta());

    if meta_spawn_timer.just_finished() {
        let old_duration = spawn_timer.duration();
        spawn_timer.set_duration((old_duration / 2).max(Duration::from_millis(100)));
    }

    if !spawn_timer.just_finished() {
        return;
    }

    let x =
        rand::random_range(-ARENA_HALF_WIDTH + WALL_THICKNESS..ARENA_HALF_WIDTH - WALL_THICKNESS);
    let y =
        rand::random_range(-ARENA_HALF_WIDTH + WALL_THICKNESS..ARENA_HALF_WIDTH - WALL_THICKNESS);
    commands.spawn((
        Sprite {
            color: Paint::None.sprite_color(),
            ..Sprite::from_atlas_image(
                asset_server.load("Enemy0.png"),
                TextureAtlas {
                    layout: enemy_layout.0.clone(),
                    index: 0,
                },
            )
        },
        Enemy { paint: Paint::None },
        Transform::from_translation(Vec3::new(x, y, ENEMY_Z)),
        Collider {
            half_size: Vec2::new(ENEMY_WIDTH / 2.0, ENEMY_HEIGHT / 2.0),
        },
        DespawnOnExit(AppState::Playing),
    ));
}

fn enemy_movement(
    time: Res<Time>,
    hero_transform: Single<&Transform, With<Hero>>,
    mut enemy_transforms: Query<&mut Transform, (With<Enemy>, Without<Hero>)>,
) {
    let speed = ENEMY_SPEED * time.delta_secs();
    let target = hero_transform.translation;
    for mut enemy_transform in &mut enemy_transforms {
        enemy_transform.translation = enemy_transform.translation.move_towards(target, speed);
    }
}

fn enemy_collision(mut query: Query<(&mut Transform, &Collider), With<Enemy>>) {
    let mut iter = query.iter_combinations_mut();
    while let Some([(mut transform1, collider1), (transform2, collider2)]) = iter.fetch_next() {
        let bounding_rect_1 = Aabb2d::new(transform1.translation.truncate(), collider1.half_size);
        let bounding_rect_2 = Aabb2d::new(transform2.translation.truncate(), collider2.half_size);

        if bounding_rect_1.intersects(&bounding_rect_2) {
            let delta = bounding_rect_1.center() - bounding_rect_2.center();
            let overlap = bounding_rect_1.half_size() + bounding_rect_2.half_size() - delta.abs();
            if overlap.x < overlap.y {
                transform1.translation.x += delta.x.signum() * overlap.x;
            } else {
                transform1.translation.y += delta.y.signum() * overlap.y;
            }
        }
    }
}

fn animate_sprites(mut sprites: Query<&mut Sprite>) {
    for mut sprite in &mut sprites {
        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = if atlas.index == 3 { 0 } else { atlas.index + 1 };
        }
    }
}

fn update_camera(
    mut camera: Single<&mut Transform, (With<Camera2d>, Without<Hero>)>,
    hero: Single<&Transform, (With<Hero>, Without<Camera2d>)>,
) {
    let Vec3 { x, y, .. } = hero.translation;
    let direction = Vec3::new(x, y, camera.translation.z);

    camera.translation = direction;
}

fn draw_paint(
    mouse_button: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera_query: Single<(&Camera, &GlobalTransform)>,
    mut paint_grid: ResMut<PaintGrid>,
    floor_image: Res<FloorImage>,
    mut images: ResMut<Assets<Image>>,
) {
    if !mouse_button.pressed(MouseButton::Left) {
        return;
    }

    let (camera, camera_transform) = *camera_query;
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Ok(world_click_position) = camera.viewport_to_world_2d(camera_transform, cursor_position)
    else {
        return;
    };

    let cell_column = floor((world_click_position.x + ARENA_HALF_WIDTH) / PAINT_CELL_SIZE);
    let cell_row = floor((ARENA_HALF_WIDTH - world_click_position.y) / PAINT_CELL_SIZE);

    let Some(mut image) = images.get_mut(&floor_image.0) else {
        return;
    };

    for col in cell_column as i32 - SPLAT_RADIUS..=cell_column as i32 + SPLAT_RADIUS {
        for row in cell_row as i32 - SPLAT_RADIUS..=cell_row as i32 + SPLAT_RADIUS {
            if !(0..paint_grid.width as i32).contains(&col)
                || !(0..paint_grid.height as i32).contains(&row)
            {
                continue;
            }

            let dc = cell_column as i32 - col;
            let dr = cell_row as i32 - row;
            if (dc * dc) + (dr * dr) > (SPLAT_RADIUS * SPLAT_RADIUS) {
                continue;
            }

            let row = row as usize;
            let col = col as usize;
            let paint_grid_width = paint_grid.width;
            paint_grid.cells[row * paint_grid_width + col] = Paint::Lead;
            let _ = image.set_color_at(col as u32, row as u32, Paint::Lead.ground_color());
        }
    }
}

fn recolor_sprite(
    paint_grid: Res<PaintGrid>,
    query: Query<
        (
            &Transform,
            &mut Sprite,
            Option<&mut Hero>,
            Option<&mut Enemy>,
        ),
        Or<(With<Hero>, With<Enemy>)>,
    >,
    mut hurt_timer: ResMut<HurtTimer>,
) {
    for (transform, mut sprite, hero, enemy) in query {
        let cell_col =
            floor((transform.translation.x + ARENA_HALF_WIDTH) / PAINT_CELL_SIZE) as usize;
        let cell_row =
            floor((ARENA_HALF_WIDTH - transform.translation.y) / PAINT_CELL_SIZE) as usize;
        if !(0..paint_grid.width).contains(&cell_col) || !(0..paint_grid.height).contains(&cell_row)
        {
            continue;
        }

        let paint_grid_width = paint_grid.width;
        let ground_color_cell = paint_grid.cells[cell_row * paint_grid_width + cell_col];

        let mut final_color = ground_color_cell.sprite_color();
        if !hurt_timer.is_finished() && hero.is_some() {
            final_color = TEXT_COLOR_RED;
        }

        if sprite.color != final_color {
            sprite.color = final_color;
            if let Some(mut hero) = hero {
                hero.paint = ground_color_cell;
            }
            if let Some(mut enemy) = enemy {
                enemy.paint = ground_color_cell;
            }
        }
    }
}

fn fire_bullets(
    hero_query: Single<(&Transform, &Hero), With<Hero>>,
    window: Single<&Window>,
    camera_query: Single<(&Camera, &GlobalTransform)>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let (camera, camera_transform) = *camera_query;
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Ok(world_cursor_position) = camera.viewport_to_world_2d(camera_transform, cursor_position)
    else {
        return;
    };

    let (hero_transform, hero_self) = *hero_query;

    let direction =
        (world_cursor_position - hero_transform.translation.truncate()).normalize_or_zero();

    commands.spawn((
        AudioPlayer::new(asset_server.load("shoot.wav")),
        PlaybackSettings::DESPAWN,
    ));
    commands.spawn((
        Transform::from_xyz(
            hero_transform.translation.x,
            hero_transform.translation.y,
            BULLET_Z,
        ),
        Sprite::from_color(hero_self.paint.sprite_color(), BULLET_SIZE),
        Velocity(direction * BULLET_SPEED),
        Bullet {
            paint: hero_self.paint,
        },
        Collider {
            half_size: Vec2::new(BULLET_WIDTH / 2.0, BULLET_WIDTH / 2.0),
        },
        DespawnOnExit(AppState::Playing),
    ));
}

fn move_bullets(time: Res<Time>, bullet_query: Query<(&Velocity, &mut Transform), With<Bullet>>) {
    for (b_velo, mut b_trans) in bullet_query {
        b_trans.translation += (b_velo.0 * time.delta_secs()).extend(0.0);
    }
}

fn bullet_despawn(bullet_query: Query<(&Transform, Entity), With<Bullet>>, mut commands: Commands) {
    for (bullet_transform, bullet_entity) in &bullet_query {
        if bullet_transform.translation.x > ARENA_HALF_WIDTH
            || bullet_transform.translation.x < -ARENA_HALF_WIDTH
            || bullet_transform.translation.y > ARENA_HALF_WIDTH
            || bullet_transform.translation.y < -ARENA_HALF_WIDTH
        {
            commands.entity(bullet_entity).despawn();
        }
    }
}

fn bullet_collision(
    bullet_query: Query<(&Transform, &Collider, Entity, &Bullet), With<Bullet>>,
    enemy_query: Query<(&Transform, &Collider, Entity, &Enemy), With<Enemy>>,
    mut commands: Commands,
    mut paint_grid: ResMut<PaintGrid>,
    floor_image: Res<FloorImage>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    for (bullet_transform, bullet_collider, bullet_entity, bullet) in &bullet_query {
        for (enemy_transform, enemy_collider, enemy_entity, enemy) in &enemy_query {
            let bullet_bounding_rect = Aabb2d::new(
                bullet_transform.translation.truncate(),
                bullet_collider.half_size,
            );
            let enemy_bounding_rect = Aabb2d::new(
                enemy_transform.translation.truncate(),
                enemy_collider.half_size,
            );

            if bullet_bounding_rect.intersects(&enemy_bounding_rect) && bullet.paint == enemy.paint
            {
                commands.entity(bullet_entity).despawn();
                commands.entity(enemy_entity).despawn();
                commands.spawn((
                    AudioPlayer::new(asset_server.load("Powerstrip_Click_2.wav")),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.1)),
                ));

                commands.spawn((
                    AudioPlayer::new(asset_server.load("shoot.wav")),
                    PlaybackSettings::DESPAWN,
                ));

                // Spawn bullets
                for _ in 0..NUM_BULLETS_SPAWN {
                    let angle = rand::random_range(0.0..TAU);
                    let direction = Vec2::from_angle(angle).normalize_or_zero();

                    commands.spawn((
                        Transform::from_xyz(
                            enemy_transform.translation.x,
                            enemy_transform.translation.y,
                            BULLET_Z,
                        ),
                        Sprite::from_color(enemy.paint.sprite_color(), BULLET_SIZE),
                        Velocity(direction * BULLET_SPEED),
                        Bullet { paint: enemy.paint },
                        Collider {
                            half_size: Vec2::new(BULLET_WIDTH / 2.0, BULLET_WIDTH / 2.0),
                        },
                        DespawnOnExit(AppState::Playing),
                    ));
                }

                // Paint the ground
                let cell_column =
                    floor((enemy_transform.translation.x + ARENA_HALF_WIDTH) / PAINT_CELL_SIZE);
                let cell_row =
                    floor((ARENA_HALF_WIDTH - enemy_transform.translation.y) / PAINT_CELL_SIZE);
                let Some(mut image) = images.get_mut(&floor_image.0) else {
                    return;
                };
                for col in cell_column as i32 - SPLAT_RADIUS..=cell_column as i32 + SPLAT_RADIUS {
                    for row in cell_row as i32 - SPLAT_RADIUS..=cell_row as i32 + SPLAT_RADIUS {
                        if !(0..paint_grid.width as i32).contains(&col)
                            || !(0..paint_grid.height as i32).contains(&row)
                        {
                            continue;
                        }

                        let dc = cell_column as i32 - col;
                        let dr = cell_row as i32 - row;
                        if (dc * dc) + (dr * dr) > (SPLAT_RADIUS * SPLAT_RADIUS) {
                            continue;
                        }

                        let row = row as usize;
                        let col = col as usize;
                        let paint_grid_width = paint_grid.width;
                        paint_grid.cells[row * paint_grid_width + col] = enemy.paint.next_color();
                        let _ = image.set_color_at(
                            col as u32,
                            row as u32,
                            enemy.paint.next_color().ground_color(),
                        );
                    }
                }
            }
        }
    }
}

fn hp_ui(mut text: Single<&mut Text, With<HpUi>>, hero: Single<&Hero>) {
    text.0 = format!("HP: {}", hero.hitpoints);
}

fn arena_state_ui(
    mut lead_text: Single<
        &mut Text,
        (
            With<LeadUi>,
            Without<IronUi>,
            Without<SilverUi>,
            Without<GoldUi>,
        ),
    >,
    mut iron_text: Single<
        &mut Text,
        (
            Without<LeadUi>,
            With<IronUi>,
            Without<SilverUi>,
            Without<GoldUi>,
        ),
    >,
    mut silver_text: Single<
        &mut Text,
        (
            Without<LeadUi>,
            Without<IronUi>,
            With<SilverUi>,
            Without<GoldUi>,
        ),
    >,
    mut gold_text: Single<
        &mut Text,
        (
            Without<LeadUi>,
            Without<IronUi>,
            Without<SilverUi>,
            With<GoldUi>,
        ),
    >,
    paint_grid: ResMut<PaintGrid>,
) {
    let mut lead_count = 0.0;
    let mut iron_count = 0.0;
    let mut silver_count = 0.0;
    let mut gold_count = 0.0;
    let mut total_count = 0.0;

    for i in 0..paint_grid.cells.len() {
        total_count += 1.0;
        let curr_paint = paint_grid.cells[i];
        match curr_paint {
            Paint::None => {}
            Paint::Lead => lead_count += 1.0,
            Paint::Iron => iron_count += 1.0,
            Paint::Silver => silver_count += 1.0,
            Paint::Gold => gold_count += 1.0,
        }
    }

    lead_text.0 = format!("Lead: {:.2}%", lead_count / total_count * 100.0);
    iron_text.0 = format!("Iron: {:.2}%", iron_count / total_count * 100.0);
    silver_text.0 = format!("Silver: {:.2}%", silver_count / total_count * 100.0);
    gold_text.0 = format!("Gold: {:.2}%", gold_count / total_count * 100.0);
}

fn on_activate_start_game(_: On<Activate>, mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Playing);
}

fn check_game_over(hero: Single<&Hero>, mut next_state: ResMut<NextState<AppState>>) {
    if hero.hitpoints <= 0 {
        next_state.set(AppState::GameOver);
    }
}

fn tick_hurt_timer(mut hurt_timer: ResMut<HurtTimer>, time: Res<Time>) {
    hurt_timer.tick(time.delta());
}
