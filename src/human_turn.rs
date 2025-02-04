use bevy::prelude::*;
use bevy::input::mouse::{MouseButtonInput, MouseButton};
use bevy::window::PrimaryWindow;
use crate::buttons::*;
use crate::components::*;
use crate::constants::*;
use crate::power::PowerEvent;
use crate::shared_systems::HighlightEvent;
use crate::resources::*;

#[derive(Debug, Event)]
struct ClickEvent(pub Vec2);

#[derive(Event)]
struct MoveEvent(pub (Entity, usize, WhichDie, Vec3));

pub struct HumanTurnPlugin;

impl Plugin for HumanTurnPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<ClickEvent>()
            .add_event::<MoveEvent>()

            .add_systems(OnEnter(GameState::HumanTurn), enable_ui)
            // ui
            .add_systems(Update,
                (execute_button_actions, mouse_watcher::<GameButtonAction>, watch_button_state_changes).chain()
                .run_if(in_state(GameState::HumanTurn))
            )
            // game play
            .add_systems(Update,
                (translate_mouse_input, interpret_click_event, move_event_handler).chain()
                .run_if(in_state(GameState::HumanTurn))
            )
            .add_systems(OnExit(GameState::HumanTurn), disable_ui)
            ;
    }
}

fn enable_ui(
    mouse_button_inputs: Res<Input<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut button_query: Query<(&mut ButtonState, &mut TextureAtlasSprite, &Transform)>,
) {
    let Ok(w) = windows.get_single() else {
        return;
    };
    let mouse_pressed = mouse_button_inputs.pressed(MouseButton::Left);

    for (mut button_state, mut button_sprite, button_transform) in &mut button_query {
        *button_state = get_button_state(w.cursor_position(), button_transform.translation, UI_BUTTON_SIZE, mouse_pressed);
        button_sprite.color = Color::WHITE;
    }
}

fn disable_ui(
    mut button_query: Query<(&mut TextureAtlasSprite, &mut ButtonState)>,
) {
    for (mut sprite, mut state) in &mut button_query {
        sprite.color = Color::rgba(1.0, 1.0, 1.0, 0.4);
        sprite.index = 0;
        *state = ButtonState::NotHovered;
    }
}

fn translate_mouse_input(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut click_events: EventWriter<ClickEvent>,
) {
    if mouse_button_input_events.iter()
        .filter(|e| e.button == MouseButton::Left && e.state.is_pressed())
        .last().is_some()
    {
        let Some(pos) = windows.get_single().map_or(None, |w| w.cursor_position()) else {
            return;
        };
        let (x, y) = (pos.x - WINDOW_SIZE / 2.0, -(pos.y - WINDOW_SIZE / 2.0));
        click_events.send(ClickEvent(Vec2::new(x, y))); 
    }
}

fn interpret_click_event(
    mut highlight_events: EventWriter<HighlightEvent>,
    mut move_events: EventWriter<MoveEvent>,
    mut click_events: EventReader<ClickEvent>,
    mut current_player_data: ResMut<CurrentPlayerData>,
    marbles_query: Query<(Entity, &Transform), (With<Marble>, With<CurrentPlayer>)>,
) {
    if let Some(click_event) = click_events.iter().last() {
        // interpret click as marble selection
        if let Some(marble) = marbles_query.iter().find_map(|(e, t)| {
                let found = click_event.0.x > t.translation.x - TILE_SIZE / 2.0 && // to the right of the left edge
                            click_event.0.x < t.translation.x + TILE_SIZE / 2.0 && // to the left of the right edge
                            click_event.0.y > t.translation.y - TILE_SIZE / 2.0 && // below the top edge
                            click_event.0.y < t.translation.y + TILE_SIZE / 2.0;   // above the bottom edge
                if found { Some(e) } else { None }
            })
        {
            if let Some(old_marble) = current_player_data.selected_marble {
                if old_marble == marble {
                    return; // ignore clicks on a marble that is already selected
                }
            }
            current_player_data.selected_marble = Some(marble);
            highlight_events.send(HighlightEvent::On);
        }
        // interpret click as move selection
        else if let Some(marble) = current_player_data.selected_marble {
            // to compare to board coordinates, we need to snap the click event to the center of a tile
            let (col, row) = (snap(click_event.0.x), snap(click_event.0.y));
            // find the move that corresponds to this click position
            let selected_move = match BOARD.into_iter().position(|(x, y)| {
                // rotate the board coordinates based on the current player
                let rot = current_player_data.player.rotate_coords((x as f32, y as f32));
                // find the board index that matches the click position
                rot == (col / TILE_SIZE, row / TILE_SIZE)
            }) {
                // find a move for this board index
                Some(clicked_board_index) => current_player_data
                    .get_moves(marble).into_iter().find(|MarbleMove{ destination, .. }| *destination == clicked_board_index),
                _ => None,
            };
            if let Some(MarbleMove{ destination, which, .. }) = selected_move {
                current_player_data.move_marble();
                move_events.send(MoveEvent((marble, destination, which, Vec3::new(col, row, Z_MARBLE))));
            } else {
                // deselect the marble if the click was anywhere unimportant
                current_player_data.selected_marble = None;
            }

            // since we didn't click on another marble and we no longer have a
            // selected marble then all highlights can be removed
            highlight_events.send(HighlightEvent::Off);
        }
        // POWERUP: ignore power up button clicks
    }
}

fn move_event_handler(
    mut commands: Commands,
    mut move_events: EventReader<MoveEvent>,
    mut marbles: Query<(Entity, &Transform, &mut Marble), With<CurrentPlayer>>,
    mut dice_data: ResMut<DiceData>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Some(MoveEvent((e, idx, which, dest))) = move_events.iter().last() {
        let (e, t, mut m) = marbles.get_mut(*e).unwrap();
        m.update_index(*idx);
        dice_data.use_die(*which, &mut commands);
        commands.entity(e).insert(Moving::new(*dest, t.translation));
        next_state.set(GameState::WaitForAnimation);
    }
}

fn execute_button_actions(
    mut action_events: EventReader<ActionEvent<GameButtonAction>>,
    mut next_state: ResMut<NextState<GameState>>,
    dice_data: Res<DiceData>,
    mut power_events: EventWriter<PowerEvent>,
) {
    for action in action_events.iter() {
        if let Some((player, index)) = match action.0 {
            GameButtonAction::Done => {
                if dice_data.dice.doubles {
                    next_state.set(GameState::DiceRoll);
                } else {
                    next_state.set(GameState::EndTurn);
                }
                None
            }
            GameButtonAction::PowerUpOne(player) => Some((player, 0)),
            GameButtonAction::PowerUpTwo(player) => Some((player, 1)),
            GameButtonAction::PowerUpThree(player) => Some((player, 2)),
        } {
            power_events.send(PowerEvent::Use{ player, index });
        }
    }
}

/// Snaps the given coordinate to the center of the tile it's inside of.
fn snap(coord: f32) -> f32 {
    // let's only deal with positive values for now
    let c = coord.abs();
    // how far away is the coordinate from the center of the tile
    let remainder = c % TILE_SIZE;
    let result = if remainder < TILE_SIZE / 2. {
        // if the coordinate is past the center (going away from the origin)
        // then snap it back to the center
        // |    X     |
        // |    <---c |
        c - remainder
    } else {
        // otherwise shift the coordinate to the next tile (going away from the
        // origin) then snap it back to the center
        // |    X    |
        // | c-------|->
        // |    <----|-c
        let shift = c + TILE_SIZE;
        shift - (shift % TILE_SIZE)
    };
    // just flip the result if the original coordinate was negative
    if coord < 0.0 && result > 0.0 {
        result * -1.0
    } else {
        result
    }
}
