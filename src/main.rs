use std::time::Duration;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

const ARENA_COLOR: Color = Color::srgb(0.0, 0.25, 0.8);
const ARENA_HALF_WIDTH: f32 = 500.0;
const WALL_THICKNESS: f32 = 30.0;
const WALL_LENGTH: f32 = ARENA_HALF_WIDTH * 2.0;
const WALL_SIZE_LONG: Vec2 = Vec2::new(WALL_LENGTH + WALL_THICKNESS, WALL_THICKNESS);
const WALL_SIZE_TALL: Vec2 = Vec2::new(WALL_THICKNESS, WALL_LENGTH);
const HERO_SPEED: f32 = 100.0;
const HERO_WIDTH: f32 = 16.0;
const HERO_HEIGHT: f32 = 24.0;
const ENEMY_SPEED: f32 = 50.0;
const ENEMY_WIDTH: f32 = 16.0;
const ENEMY_HEIGHT: f32 = 24.0;

#[derive(Component)]
struct Hero;

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct Wall;

#[derive(Component)]
struct Health {
    level: f32,
}

#[derive(Component)]
struct Collider {
    half_size: Vec2,
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
        .add_systems(Startup, setup)
        .add_systems(Update, (hero_movement, wall_collision).chain())
        .add_systems(Update, enemy_spawn.run_if(on_timer(Duration::from_secs(2))))
        .add_systems(Update, enemy_movement)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 2.0,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // Hero
    commands.spawn((
        Sprite {
            image: asset_server.load("Man1.png"),
            ..Default::default()
        },
        Hero,
        Collider {
            half_size: Vec2::new(HERO_WIDTH / 2.0, HERO_HEIGHT / 2.0),
        },
    ));

    // Arena
    // Top
    commands.spawn((
        Sprite::from_color(ARENA_COLOR, WALL_SIZE_LONG),
        Transform::from_translation(Vec3::new(0.0, WALL_LENGTH / 2.0, 0.0)),
        Collider {
            half_size: WALL_SIZE_LONG / 2.0,
        },
        Wall,
    ));
    // Bottom
    commands.spawn((
        Sprite::from_color(ARENA_COLOR, WALL_SIZE_LONG),
        Transform::from_translation(Vec3::new(0.0, -(WALL_LENGTH / 2.0), 0.0)),
        Collider {
            half_size: WALL_SIZE_LONG / 2.0,
        },
        Wall,
    ));
    // Right
    commands.spawn((
        Sprite::from_color(ARENA_COLOR, WALL_SIZE_TALL),
        Transform::from_translation(Vec3::new(WALL_LENGTH / 2.0, 0.0, 0.0)),
        Collider {
            half_size: WALL_SIZE_TALL / 2.0,
        },
        Wall,
    ));
    // Left
    commands.spawn((
        Sprite::from_color(ARENA_COLOR, WALL_SIZE_TALL),
        Transform::from_translation(Vec3::new(-(WALL_LENGTH / 2.0), 0.0, 0.0)),
        Collider {
            half_size: WALL_SIZE_TALL / 2.0,
        },
        Wall,
    ));
}

fn hero_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut heroes: Query<&mut Transform, With<Hero>>,
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

    for mut transform in &mut heroes {
        transform.translation += (direction * HERO_SPEED * time.delta_secs()).extend(0.0);
    }
}

fn wall_collision(
    mut heroes: Query<(&mut Transform, &Collider), With<Hero>>,
    walls: Query<(&Transform, &Collider), (With<Wall>, Without<Hero>)>,
) {
    for (mut hero_transform, hero_collider) in &mut heroes {
        for (wall_transform, wall_collider) in &walls {
            let hero_bounding_rect = Aabb2d::new(
                hero_transform.translation.truncate(),
                hero_collider.half_size,
            );
            let wall_bounding_rect = Aabb2d::new(
                wall_transform.translation.truncate(),
                wall_collider.half_size,
            );

            if hero_bounding_rect.intersects(&wall_bounding_rect) {
                let delta = hero_bounding_rect.center() - wall_bounding_rect.center();
                let overlap =
                    hero_bounding_rect.half_size() + wall_bounding_rect.half_size() - delta.abs();
                if overlap.x < overlap.y {
                    hero_transform.translation.x += delta.x.signum() * overlap.x;
                } else {
                    hero_transform.translation.y += delta.y.signum() * overlap.y;
                }
            }
        }
    }
}

fn enemy_spawn(mut commands: Commands, asset_server: Res<AssetServer>) {
    let x =
        rand::random_range(-ARENA_HALF_WIDTH + WALL_THICKNESS..ARENA_HALF_WIDTH - WALL_THICKNESS);
    let y =
        rand::random_range(-ARENA_HALF_WIDTH + WALL_THICKNESS..ARENA_HALF_WIDTH - WALL_THICKNESS);
    commands.spawn((
        Sprite {
            image: asset_server.load("Zombie.png"),
            ..Default::default()
        },
        Enemy,
        Transform::from_translation(Vec3::new(x, y, 0.0)),
        Collider {
            half_size: Vec2::new(ENEMY_WIDTH / 2.0, ENEMY_HEIGHT / 2.0),
        },
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
