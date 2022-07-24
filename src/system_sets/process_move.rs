use bevy::prelude::*;
use crate::components::*;
use crate::constants::*;
use crate::resources::*;

pub fn check_for_capture(
    mut commands: Commands,
    current_player_data: Res<CurrentPlayerData>,
    selected_marble: Query<(Entity, &Marble), (With<CurrentPlayer>, With<SelectedMarble>)>,
    mut opponent_marbles: Query<(Entity, &mut Marble, &Transform, &Player), Without<CurrentPlayer>>,
) {
    let (e, cur) = selected_marble.single();

    // we don't capture in the home row
    if cur.index >= FIRST_HOME_INDEX && cur.index <= LAST_HOME_INDEX {
        return;
    }

    if let Some((entity, mut oppenent_marble, transform, opponent)) = opponent_marbles.iter_mut()
        // do not check opponent marbles in their home row or at their base
        .filter(|(_, opp, _, _)| opp.index < FIRST_HOME_INDEX || opp.index == CENTER_INDEX)
        // find an opponent marble at the same index as the marble just moved by the current player
        .find(|(_, opp, _, p)| Player::is_same_index(current_player_data.player, cur.index, **p, opp.index))
    {
        println!("{:?} {:?} @ {} captured {:?} {:?} @ {}",
            current_player_data.player, e, cur.index,
            opponent, entity, oppenent_marble.index
        );
        oppenent_marble.index = BOARD.len();
        commands.entity(entity).insert(Moving::new(oppenent_marble.origin, transform.translation));
    }
}

pub fn check_for_winner(
    mut state: ResMut<State<GameState>>,
    dice_data: Res<DiceData>,
    marbles: Query<&Marble, With<CurrentPlayer>>,
    current_player_data: Res<CurrentPlayerData>,
) {
    if marbles.iter()
        .find(|m| !(FIRST_HOME_INDEX..=LAST_HOME_INDEX).contains(&m.index))
        .is_some()
    {
        // not a winner
        match (dice_data.die_1_side, dice_data.die_2_side) {
            (Some(_), None) | (None, Some(_)) => state.set(GameState::TurnSetup).unwrap(),
            (None, None) => if dice_data.doubles {
                state.set(GameState::DiceRoll).unwrap();
            } else {
                state.set(GameState::NextPlayer).unwrap();
            }
            _ => unreachable!(),
        }
    } else {
        // winner
        println!("{:?} Wins!", current_player_data.player);
        state.set(GameState::GameEnd).unwrap();
    }
}

pub fn clear_selected_marble(
    mut commands: Commands,
    selected_marble: Query<Entity, With<SelectedMarble>>,
) {
    let selected_marble = selected_marble.single();
    commands.entity(selected_marble).remove::<SelectedMarble>();
}
