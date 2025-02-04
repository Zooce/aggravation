use std::collections::HashMap;

use bevy::prelude::*;
use crate::buttons::*;
use crate::components::*;
use crate::computer_turn::*;
use crate::constants::*;
use crate::end_turn::*;
use crate::choose_color::ChooseColorPlugin;
use crate::dice_roll::DiceRollPlugin;
use crate::human_turn::HumanTurnPlugin;
use crate::next_player::*;
use crate::power::PowerBar;
use crate::power::PowerUpHighlightImages;
use crate::power::PowerUpPlugin;
use crate::power::PowerUpSpriteImages;
use crate::process::ProcessMovePlugin;
use crate::resources::*;
use crate::shared_systems::*;
use crate::turn_setup::*;
use rand::{Rng, thread_rng};
use rand::distributions::Uniform;

pub struct VexationPlugin;

impl Plugin for VexationPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<HighlightEvent>()
            .add_event::<MarbleAnimationDoneEvent>()
            .add_event::<ActionEvent<GameButtonAction>>()

            // game play enter
            .add_systems(OnEnter(GameState::GameStart), create_game)

            // game play exit
            .add_systems(Update, game_end.run_if(in_state(GameState::GameEnd)))
            .add_systems(OnExit(GameState::GameEnd), destroy_game)

            // --- states + systems -- TODO: move each to their own plugin to keep things smaller?

            .configure_sets(Update, SharedSystemSet.run_if(should_run_shared_systems))
            // shared systems
            .add_systems(Update, (
                animate_marble_moves,
                highlighter,
                animate_tile_highlights,
                dim_used_die
                ).in_set(SharedSystemSet)
            )
            .add_plugins(PowerUpPlugin)

            // next player
            .add_systems(Update, (choose_next_player, show_or_hide_buttons, next_player_setup).chain()
                .run_if(in_state(GameState::NextPlayer))
            )

            // turn setup
            .add_systems(Update, (calc_possible_moves, count_moves, turn_setup_complete).chain()
                .run_if(in_state(GameState::TurnSetup))
            )

            // computer turn
            .add_systems(
                OnEnter(GameState::ComputerTurn),
                (clear_animation_events, computer_choose_move).chain()
            )
            .add_systems(Update, computer_move_buffer.run_if(in_state(GameState::ComputerTurn)))

            .add_systems(Update, wait_for_marble_animation.run_if(in_state(GameState::WaitForAnimation)))

            .add_plugins(ChooseColorPlugin)
            .add_plugins(DiceRollPlugin)
            .add_plugins(HumanTurnPlugin)
            .add_plugins(ProcessMovePlugin)

            // end turn
            .add_systems(Update, end_turn.run_if(in_state(GameState::EndTurn)))
            ;
    }
}

pub fn create_game(
    mut commands: Commands,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut textures: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    // insert resources
    commands.insert_resource(BufferTimer(Timer::from_seconds(1.0, TimerMode::Once)));
    commands.insert_resource(ComputerTurnTimers{
        move_timer: Timer::from_seconds(COMPUTER_MOVE_TIMER_SECS, TimerMode::Once),
        buffer_timer: Timer::from_seconds(COMPUTER_BUFFER_TIMER_SECS, TimerMode::Once),
    });
    commands.insert_resource(RollAnimationTimer(Timer::from_seconds(1.5, TimerMode::Once)));
    commands.insert_resource(GameData{
        players: HashMap::from([
            (Player::Red, PlayerData::default()),
            (Player::Green, PlayerData::default()),
            (Player::Blue, PlayerData::default()),
            (Player::Yellow, PlayerData::default()),
        ]),
    });
    commands.insert_resource(PowerUpSpriteImages{
        roll_again: load_sprite_sheet("power-ups/roll-again-sheet.png", TILE_BUTTON_SIZE, (3, 1), &asset_server, &mut textures, &mut texture_atlases),
        double_dice: load_sprite_sheet("power-ups/double-dice-sheet.png", TILE_BUTTON_SIZE, (3, 1), &asset_server, &mut textures, &mut texture_atlases),
        evade_capture: load_sprite_sheet("power-ups/evade-capture-sheet.png", TILE_BUTTON_SIZE, (3, 1), &asset_server, &mut textures, &mut texture_atlases),
        self_jump: load_sprite_sheet("power-ups/self-jump-sheet.png", TILE_BUTTON_SIZE, (3, 1), &asset_server, &mut textures, &mut texture_atlases),
        capture_nearest: load_sprite_sheet("power-ups/capture-nearest-sheet.png", TILE_BUTTON_SIZE, (3, 1), &asset_server, &mut textures, &mut texture_atlases),
        home_run: load_sprite_sheet("power-ups/home-run-sheet.png", TILE_BUTTON_SIZE, (3, 1), &asset_server, &mut textures, &mut texture_atlases),
    });
    commands.insert_resource(PowerUpHighlightImages{
        evading: asset_server.load("power-ups/evade-capture-highlight.png"),
        self_jumping: asset_server.load("power-ups/self-jump-highlight.png"),
    });

    // pick the first player randomly
    let mut rng = thread_rng();
    let die = Uniform::new_inclusive(0u8, 3u8);
    let current_player: Player = rng.sample(die).into();
    commands.insert_resource(CurrentPlayerData::new(current_player));

    // background
    let mut game_play_entities = GamePlayEntities::default();
    game_play_entities.board_entities.push(commands.spawn((
        Sprite{
            image: asset_server.load("background.png"),
            ..default()
        },
        Transform::from_xyz(0., 0., Z_BACKGROUND),
    )).id());
    // board
    game_play_entities.board_entities.push(commands.spawn((
        Sprite{
            image: asset_server.load("board.png"),
            ..default()
        },
        Transform::from_xyz(0., 0., Z_BOARD),
    )).id());
    // TODO: animate power up slots onto the board AFTER the player chooses their color
    // animation idea:
    // → ←
    // → ←
    game_play_entities.board_entities.push(commands.spawn((
        Sprite{
            image: asset_server.load("power-up-slots.png"),
            ..default()
        },
        Transform::from_xyz(0., 0., Z_POWER_UP),
    )).id());
    // TODO: animate power bars onto the board AFTER the player chooses their color
    // animation idea:
    // ↓↓
    // ↑↑
    game_play_entities.board_entities.push(commands.spawn((
        Sprite{
            image: asset_server.load("power-bars.png"),
            ..default()
        }, Transform::from_xyz(0., 0., Z_POWER_BAR),
    )).id());
    let power_fill = asset_server.load("power-fill.png");
    for ((x, y), player) in &[
        ((-7.75, 0.), Player::Red),
        ((-7.75, -8.), Player::Yellow),
        ((7.75, 0.), Player::Green),
        ((7.75, -8.), Player::Blue)
    ] {
        game_play_entities.board_entities.push(commands.spawn((
            Sprite{
                image: power_fill.clone(),
                ..default()
            },
            Transform::from_xyz(x * TILE_SIZE, y * TILE_SIZE + 2., Z_POWER_FILL),
            PowerBar::new(y * TILE_SIZE + 2.),
            *player,
        )).id());
    }
    // human player turn end UI button
    let sprite_sheet = texture_atlases.add(TextureAtlas::from_grid(
        asset_server.load("buttons/done_button.png"), UI_BUTTON_SIZE, 3, 1, None, None
    ));
    game_play_entities.board_entities.push(commands
        .spawn(sprite_sheet_button_bundle(
            sprite_sheet,
            Transform::from_xyz(0.0, (-WINDOW_SIZE / 2.0) + TILE_SIZE, Z_UI),
            ButtonAction(ActionEvent(GameButtonAction::Done)),
            Visibility::Hidden,
            ButtonState::NotHovered,
            ButtonSize(UI_BUTTON_SIZE),
        ))
        .insert(Hidable)
        .id()
    );
    commands.insert_resource(game_play_entities);

    // highlight data
    commands.insert_resource(HighlightData{
        marble_texture: asset_server.load("marble-highlight.png"),
        tile_texture: asset_server.load("tile-highlight.png"),
    });

    // TODO: create all marbles at the center and animate them to their bases - AFTER choose color system
    // marbles
    let red_marble = asset_server.load("marbles/red-marble.png");
    let green_marble = asset_server.load("marbles/green-marble.png");
    let blue_marble = asset_server.load("marbles/blue-marble.png");
    let yellow_marble = asset_server.load("marbles/yellow-marble.png");
    for (x, y) in &[(2.5, 3.5), (2.5, 4.5), (3.5, 3.), (3.5, 4.), (3.5, 5.)] {
        // green marbles
        let origin = Transform::from_xyz(x * TILE_SIZE, y * TILE_SIZE, Z_MARBLE);
        let mut green = commands
            .spawn((
                SpriteBundle{
                    texture: green_marble.clone(),
                    transform: origin,
                    ..default()
                },
                Marble::new(origin.translation),
                Player::Green,
            ));
        if current_player == Player::Green {
            green.insert(CurrentPlayer);
        }
        // yellow marbles
        let origin = Transform::from_xyz(-x * TILE_SIZE, -y * TILE_SIZE, Z_MARBLE);
        let mut yellow = commands
            .spawn((
                SpriteBundle{
                    texture: yellow_marble.clone(),
                    transform: origin,
                    ..default()
                },
                Marble::new(origin.translation),
                Player::Yellow,
            ));
        if current_player == Player::Yellow {
            yellow.insert(CurrentPlayer);
        }
        // red marbles
        let origin = Transform::from_xyz(-y * TILE_SIZE, x * TILE_SIZE, Z_MARBLE);
        let mut red = commands
            .spawn((
                SpriteBundle{
                    texture: red_marble.clone(),
                    transform: origin,
                    ..default()
                },
                Marble::new(origin.translation),
                Player::Red,
            ));
        if current_player == Player::Red {
            red.insert(CurrentPlayer);
        }
        // blue marbles
        let origin = Transform::from_xyz(y * TILE_SIZE, -x * TILE_SIZE, Z_MARBLE);
        let mut blue = commands
            .spawn((
                SpriteBundle{
                    texture: blue_marble.clone(),
                    transform: origin,
                    ..default()
                },
                Marble::new(origin.translation),
                Player::Blue,
            ));
        if current_player == Player::Blue {
            blue.insert(CurrentPlayer);
        }
    }

    // die sprite sheet
    let die_sheet_handle = texture_atlases.add(TextureAtlas::from_grid(
        asset_server.load("die-sheet.png"), Vec2::new(32.0, 32.0), 6, 1, None, None
    ));
    let die_1 = commands
        .spawn((
            SpriteSheetBundle{
                texture_atlas: die_sheet_handle.clone(),
                visibility: Visibility::Hidden,
                transform: Transform::from_xyz(0.0, 0.0, Z_DICE),
                ..default()
            },
            Die{
                location: Vec3::new(0.0, 0.0, Z_DICE),
                timer: Timer::from_seconds(0.1, TimerMode::Repeating),
            },
        ))
        .id();
    let die_2 = commands
        .spawn((
            SpriteSheetBundle{
                texture_atlas: die_sheet_handle.clone(),
                visibility: Visibility::Hidden,
                transform: Transform::from_xyz(0.0, 0.0, Z_DICE),
                ..default()
            },
            Die{
                location: Vec3::new(0.0, 0.0, Z_DICE),
                timer: Timer::from_seconds(0.1, TimerMode::Repeating),
            }
        ))
        .id();

    commands.insert_resource(DiceData {
        die_1,
        die_2,
        die_sheet_handle,
        dice: Dice::default(),
    });

    next_state.set(GameState::ChooseColor);
}

pub fn game_end(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::MainMenu);
}

pub fn destroy_game(
    mut commands: Commands,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut next_state: ResMut<NextState<GameState>>,
    dice_data: Res<DiceData>,
    game_play_entities: Res<GamePlayEntities>,
    human_player: Res<HumanPlayer>,
    marbles: Query<Entity, With<Marble>>,
    mut game_data: ResMut<GameData>,
) {
    for e in &game_play_entities.board_entities {
        commands.entity(*e).despawn_recursive(); // FIXME: there's a panic here because the entity doesn't exist (try commands.get_entity() + figure out why that entity doesn't exist)
    }
    for player in [Player::Red, Player::Green, Player::Blue, Player::Yellow] {
        let player_data = game_data.players.get_mut(&player).unwrap();
        for i in 0..3 {
            if let Some((_, e)) = player_data.use_power_up(i) {
                commands.entity(e).despawn();
            }
        }
    }
    commands.entity(human_player.human_indicator).despawn();
    commands.entity(dice_data.die_1).despawn();
    commands.entity(dice_data.die_2).despawn();

    texture_atlases.remove(dice_data.die_sheet_handle.id());

    commands.remove_resource::<GamePlayEntities>();
    commands.remove_resource::<BufferTimer>();
    commands.remove_resource::<ComputerTurnTimers>();
    commands.remove_resource::<RollAnimationTimer>();
    commands.remove_resource::<CurrentPlayerData>();
    commands.remove_resource::<DiceData>();
    commands.remove_resource::<HighlightData>();

    for marble in &marbles {
        commands.entity(marble).despawn_recursive();
    }

    next_state.set(GameState::MainMenu);
}
