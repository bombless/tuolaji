use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const GRID_WIDTH: i32 = 16;
const GRID_HEIGHT: i32 = 10;
const TILE_SIZE: f32 = 56.0;
const TRACTOR_SIZE: f32 = 40.0;
const TRACTOR_SPEED: f32 = 360.0;
const CROP_GROW_SECONDS: f32 = 4.0;
const TITLE_BASE: &str = "Tuolaji Farm (Bevy 0.18.1)";
const TITLE_CONTROLS: &str = "Controls: WASD/Arrows move, R reset";

const COLOR_GRASS: Color = Color::srgb(0.24, 0.62, 0.26);
const COLOR_GROWING: Color = Color::srgb(0.42, 0.36, 0.22);
const COLOR_RIPE: Color = Color::srgb(0.93, 0.77, 0.19);
const COLOR_TRACTOR: Color = Color::srgb(0.80, 0.16, 0.12);

#[derive(Component)]
struct Tractor;

#[derive(Component, Clone, Copy)]
struct TileCoord {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TileState {
    Grass,
    Growing,
    Ripe,
}

#[derive(Clone, Copy)]
struct TileData {
    state: TileState,
    grow_elapsed: f32,
}

#[derive(Resource)]
struct Field {
    width: i32,
    height: i32,
    tiles: Vec<TileData>,
    sowed: u32,
    harvested: u32,
    score: u32,
}

impl Field {
    fn new(width: i32, height: i32) -> Self {
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for _ in 0..(width * height) {
            tiles.push(TileData {
                state: TileState::Grass,
                grow_elapsed: 0.0,
            });
        }

        Self {
            width,
            height,
            tiles,
            sowed: 0,
            harvested: 0,
            score: 0,
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }

        Some((y * self.width + x) as usize)
    }

    fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut TileData> {
        let idx = self.idx(x, y)?;
        self.tiles.get_mut(idx)
    }

    fn get(&self, x: i32, y: i32) -> Option<&TileData> {
        let idx = self.idx(x, y)?;
        self.tiles.get(idx)
    }

    fn world_half_size(&self) -> Vec2 {
        Vec2::new(
            self.width as f32 * TILE_SIZE * 0.5,
            self.height as f32 * TILE_SIZE * 0.5,
        )
    }

    fn ripe_count(&self) -> usize {
        self.tiles
            .iter()
            .filter(|tile| tile.state == TileState::Ripe)
            .count()
    }

    fn reset(&mut self) {
        for tile in &mut self.tiles {
            tile.state = TileState::Grass;
            tile.grow_elapsed = 0.0;
        }

        self.sowed = 0;
        self.harvested = 0;
        self.score = 0;
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.08, 0.15, 0.11)))
        .insert_resource(Field::new(GRID_WIDTH, GRID_HEIGHT))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: TITLE_BASE.into(),
                resolution: (1280, 800).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                move_tractor,
                tractor_interact_with_field,
                grow_crops,
                refresh_tile_colors,
                reset_game,
                update_window_title,
            )
                .chain(),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Field tiles
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let world_pos = tile_to_world(x, y);
            commands.spawn((
                Sprite::from_color(COLOR_GRASS, Vec2::splat(TILE_SIZE - 2.0)),
                Transform::from_translation(world_pos.extend(0.0)),
                TileCoord { x, y },
            ));
        }
    }

    // Tractor
    commands.spawn((
        Sprite::from_color(COLOR_TRACTOR, Vec2::new(TRACTOR_SIZE, TRACTOR_SIZE * 0.8)),
        Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)),
        Tractor,
    ));
}

fn move_tractor(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    field: Res<Field>,
    mut tractor_query: Query<&mut Transform, With<Tractor>>,
) {
    let Some(mut transform) = tractor_query.iter_mut().next() else {
        return;
    };

    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction.length_squared() > 0.0 {
        let dir = direction.normalize();
        transform.translation += (dir * TRACTOR_SPEED * time.delta_secs()).extend(0.0);

        // Rotate front side toward moving direction.
        transform.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
    }

    let half = field.world_half_size();
    let min_x = -half.x + TRACTOR_SIZE * 0.5;
    let max_x = half.x - TRACTOR_SIZE * 0.5;
    let min_y = -half.y + TRACTOR_SIZE * 0.5;
    let max_y = half.y - TRACTOR_SIZE * 0.5;

    transform.translation.x = transform.translation.x.clamp(min_x, max_x);
    transform.translation.y = transform.translation.y.clamp(min_y, max_y);
}

fn tractor_interact_with_field(field: ResMut<Field>, tractor_query: Query<&Transform, With<Tractor>>) {
    let Some(transform) = tractor_query.iter().next() else {
        return;
    };

    let tractor_pos = transform.translation.truncate();
    let Some((x, y)) = world_to_tile(tractor_pos) else {
        return;
    };

    let mut field = field;
    if let Some(tile) = field.get_mut(x, y) {
        match tile.state {
            TileState::Grass => {
                tile.state = TileState::Growing;
                tile.grow_elapsed = 0.0;
                field.sowed += 1;
                field.score += 1;
            }
            TileState::Growing => {}
            TileState::Ripe => {
                tile.state = TileState::Grass;
                tile.grow_elapsed = 0.0;
                field.harvested += 1;
                field.score += 10;
            }
        }
    }
}

fn grow_crops(time: Res<Time>, mut field: ResMut<Field>) {
    let dt = time.delta_secs();

    for tile in &mut field.tiles {
        if tile.state == TileState::Growing {
            tile.grow_elapsed += dt;
            if tile.grow_elapsed >= CROP_GROW_SECONDS {
                tile.state = TileState::Ripe;
                tile.grow_elapsed = CROP_GROW_SECONDS;
            }
        }
    }
}

fn refresh_tile_colors(field: Res<Field>, mut tile_query: Query<(&TileCoord, &mut Sprite), Without<Tractor>>) {
    for (coord, mut sprite) in &mut tile_query {
        if let Some(tile) = field.get(coord.x, coord.y) {
            sprite.color = match tile.state {
                TileState::Grass => COLOR_GRASS,
                TileState::Growing => COLOR_GROWING,
                TileState::Ripe => COLOR_RIPE,
            };
        }
    }
}

fn reset_game(
    keys: Res<ButtonInput<KeyCode>>,
    mut field: ResMut<Field>,
    mut tractor_query: Query<&mut Transform, With<Tractor>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    field.reset();

    if let Some(mut transform) = tractor_query.iter_mut().next() {
        transform.translation = Vec3::new(0.0, 0.0, 2.0);
        transform.rotation = Quat::IDENTITY;
    }
}

fn update_window_title(field: Res<Field>, mut window_q: Query<&mut Window, With<PrimaryWindow>>) {
    if !field.is_changed() {
        return;
    }

    let Some(mut window) = window_q.iter_mut().next() else {
        return;
    };

    window.title = format!(
        "{} | Sowed:{} Harvested:{} Ripe:{} Score:{} | {}",
        TITLE_BASE,
        field.sowed,
        field.harvested,
        field.ripe_count(),
        field.score,
        TITLE_CONTROLS
    );
}

fn tile_to_world(x: i32, y: i32) -> Vec2 {
    let origin_x = -(GRID_WIDTH as f32 * TILE_SIZE * 0.5) + TILE_SIZE * 0.5;
    let origin_y = -(GRID_HEIGHT as f32 * TILE_SIZE * 0.5) + TILE_SIZE * 0.5;

    Vec2::new(origin_x + x as f32 * TILE_SIZE, origin_y + y as f32 * TILE_SIZE)
}

fn world_to_tile(pos: Vec2) -> Option<(i32, i32)> {
    let half_w = GRID_WIDTH as f32 * TILE_SIZE * 0.5;
    let half_h = GRID_HEIGHT as f32 * TILE_SIZE * 0.5;

    let x = ((pos.x + half_w) / TILE_SIZE).floor() as i32;
    let y = ((pos.y + half_h) / TILE_SIZE).floor() as i32;

    if x < 0 || y < 0 || x >= GRID_WIDTH || y >= GRID_HEIGHT {
        None
    } else {
        Some((x, y))
    }
}
