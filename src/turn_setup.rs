use bevy::prelude::*;
use crate::components::*;
use crate::constants::*;
use crate::shared_systems::HighlightEvent;
use crate::resources::*;
use std::collections::BTreeSet;

pub fn calc_possible_moves(
    dice_data: Res<DiceData>,
    marbles: Query<(Entity, &Marble), With<CurrentPlayer>>,
    mut current_player_data: ResMut<CurrentPlayerData>,
    game_data: Res<GameData>,
) {
    let player_data = game_data.players.get(&current_player_data.player).unwrap();
    let mut possible_moves = BTreeSet::new(); // so we disregard duplicates
    
    if player_data.power_up_status.home_run {
        let open_home_indexes: Vec<usize> = (FIRST_HOME_INDEX..=LAST_HOME_INDEX).into_iter()
            .filter_map(|i| match marbles.iter().find(|(_, m)| m.index == i) {
                Some(_) => None,
                None => Some(i),
            })
            .collect();
        marbles.iter()
            // home runs are only for marbles that are not already home
            .filter(|(_, m)| !(FIRST_HOME_INDEX..=LAST_HOME_INDEX).contains(&m.index))
            // add each open home index as a possible move
            .for_each(|(e, _)| open_home_indexes.iter().for_each(|&i| {
                possible_moves.insert((e, vec![i], WhichDie::Neither));
            }));
    }

    if !dice_data.dice.is_empty() {
        for (entity, marble) in &marbles {
            // exit base
            if marble.index == BOARD.len() {
                base_exit_rules(&dice_data.dice, entity, &mut possible_moves);
                continue;
            }

            // exit center
            if marble.index == CENTER_INDEX {
                center_exit_rules(&dice_data.dice, entity, &mut possible_moves);
                continue;
            }

            // basic moves
            basic_rules(&dice_data.dice, entity, marble, &mut possible_moves);
        }
    }

    // filter out moves that violate the self-hop rules and moves that land on "evading" opponents
    current_player_data.possible_moves = possible_moves.into_iter()
        .filter_map(|(entity, path, which)| {
            match marbles.iter()
                .filter(|(e, _)| *e != entity) // no need to compare the same marbles
                .find(|(_, other_marble)| {
                    // if we're allowed to jump over our own marbles find one where we land on it
                    if player_data.power_up_status.jump_self_turns > 0 {
                        other_marble.index == *path.last().unwrap()
                    }
                    // look for another one of our marbles along the path of this move
                    else {
                        path.iter().any(|i| other_marble.index == *i)
                    }
                })
                // POWERUP: filter out moves that land on opponents who are currently "evading"
            {
                Some(_) => None, // we found one of our other marbles in the way of this move
                None => Some((entity, *path.last().unwrap(), which))
            }
        })
        .collect();
}

pub fn count_moves(
    mut game_data: ResMut<GameData>,
    current_player_data: Res<CurrentPlayerData>,
) {
    let count = current_player_data.possible_moves.len() as u8;
    game_data.players.get_mut(&current_player_data.player).unwrap().turn_move_count += count;
}

pub fn turn_setup_complete(
    mut state: ResMut<State<GameState>>,
    human_player: Res<HumanPlayer>,
    current_player_data: Res<CurrentPlayerData>,
    mut highlight_events: EventWriter<HighlightEvent>,
) {
    // rehighlight the selected marble if there is one - this would be because
    // the current player used a power up that changed the possible moves
    if current_player_data.selected_marble.is_some() {
        highlight_events.send(HighlightEvent::On);
    }
    if human_player.color == current_player_data.player {
        state.set(GameState::HumanTurn).unwrap();
    } else {
        state.set(GameState::ComputerTurn).unwrap();
    }
}

/// Calculates the path from a starting index into the center index. This will
/// return `None` if the end index is not one index past a center entrance
/// index. If a path is returned it requires the use of both dice (i.e. a marble
/// can only land on the center space using an exact roll with both dice).
fn enter_center_path(start: usize, end: usize) -> Option<Vec<usize>> {
    if CENTER_ENTRANCE_INDEXES.contains(&(end - 1)) {
        let mut path: Vec<_> = (start..=end - 1).collect();
        path.push(CENTER_INDEX);
        Some(path)
    } else {
        None
    }
}

fn base_exit_rules(
    dice: &Dice,
    entity: Entity,
    possible_moves: &mut BTreeSet<(Entity, Vec<usize>, WhichDie)>,
) {
    if dice.one == Some(1) || dice.one == Some(6) {
        possible_moves.insert((entity, vec![START_INDEX], WhichDie::One)); // exit with die 1...
        if let Some(two) = dice.two {
            let dest = START_INDEX + (two * dice.multiplier) as usize;
            possible_moves.insert((entity, (START_INDEX..=dest).collect(), WhichDie::Both)); // ...then move with die 2...or...
            if let Some(center_path) = enter_center_path(START_INDEX, dest) {
                possible_moves.insert((entity, center_path, WhichDie::Both)); // ...move to center with die 2
            }
        }
    }
    if dice.two == Some(1) || dice.two == Some(6) {
        possible_moves.insert((entity, vec![START_INDEX], WhichDie::Two)); // exit with die 2...
        if let Some(one) = dice.one {
            let dest = START_INDEX + (one * dice.multiplier) as usize;
            possible_moves.insert((entity, (START_INDEX..=dest).collect(), WhichDie::Both)); // ...then move with die 1...or...
            if let Some(center_path) = enter_center_path(START_INDEX, dest) {
                possible_moves.insert((entity, center_path, WhichDie::Both)); //...move to center with die 1
            }
        }
    }
}

fn center_exit_rules(
    dice: &Dice,
    entity: Entity,
    possible_moves: &mut BTreeSet<(Entity, Vec<usize>, WhichDie)>,
) {
    match (dice.one, dice.two) {
        (Some(1), Some(1)) => {
            possible_moves.insert((entity, vec![CENTER_EXIT_INDEX], WhichDie::One));
            possible_moves.insert((entity, vec![CENTER_EXIT_INDEX], WhichDie::Two));
            possible_moves.insert((entity, vec![CENTER_EXIT_INDEX, CENTER_EXIT_INDEX + dice.multiplier as usize], WhichDie::Both));
        }
        (Some(1), Some(d2)) => {
            possible_moves.insert((entity, vec![CENTER_EXIT_INDEX], WhichDie::One));
            possible_moves.insert((entity, (CENTER_EXIT_INDEX..=CENTER_EXIT_INDEX + (d2 * dice.multiplier) as usize).collect(), WhichDie::Both));
        }
        (Some(d1), Some(1)) => {
            possible_moves.insert((entity, vec![CENTER_EXIT_INDEX], WhichDie::Two));
            possible_moves.insert((entity, (CENTER_EXIT_INDEX..=CENTER_EXIT_INDEX + (d1 * dice.multiplier) as usize).collect(), WhichDie::Both));
        }
        (Some(1), None) => { possible_moves.insert((entity, vec![CENTER_EXIT_INDEX], WhichDie::One)); }
        (None, Some(1)) => { possible_moves.insert((entity, vec![CENTER_EXIT_INDEX], WhichDie::Two)); }
        _ => {} // no exit
    }
}

fn basic_rules(
    dice: &Dice,
    entity: Entity,
    marble: &Marble,
    possible_moves: &mut BTreeSet<(Entity, Vec<usize>, WhichDie)>,
) {
    let mut basic_moves = BTreeSet::new();
    match (dice.one, dice.two) {
        (Some(d1), Some(d2)) => {
            basic_moves.insert((entity, (marble.index + 1..=marble.index + (d1 * dice.multiplier) as usize).collect(), WhichDie::One));
            basic_moves.insert((entity, (marble.index + 1..=marble.index + (d2 * dice.multiplier) as usize).collect(), WhichDie::Two));
            basic_moves.insert((entity, (marble.index + 1..=marble.index + ((d1 + d2) * dice.multiplier) as usize).collect(), WhichDie::Both));

            if let Some(center_path) = enter_center_path(marble.index, marble.index + ((d1 + d2) * dice.multiplier) as usize) {
                basic_moves.insert((entity, center_path, WhichDie::Both));
            }
        }
        (Some(d1), None) => {
            basic_moves.insert((entity, (marble.index + 1..=marble.index + (d1 * dice.multiplier) as usize).collect(), WhichDie::One));
        }
        (None, Some(d2)) => {
            basic_moves.insert((entity, (marble.index + 1..=marble.index + (d2 * dice.multiplier) as usize).collect(), WhichDie::Two));
        }
        _ => unreachable!(),
    }

    // filter out moves that don't make sense
    basic_moves = basic_moves.into_iter().filter(|(_, path, _)| {
        let dest = *path.last().unwrap();
        dest <= LAST_HOME_INDEX // destination must be a valid board space
            || (dest == CENTER_INDEX // the center space is okay as long as...
                // ...the marble was not at the end of the home row (this means the path will only be [CENTER_INDEX]) AND...
                && marble.index != LAST_HOME_INDEX
                // ...the path doesn't go through the home row
                && !path.iter().any(|i| *i >= FIRST_HOME_INDEX && *i <= LAST_HOME_INDEX))
    }).collect();

    possible_moves.append(&mut basic_moves);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_base_exit_moves() {
        let dice = Dice::new(1, 6);
        let mut moves = BTreeSet::new();
        base_exit_rules(&dice, Entity::from_raw(12), &mut moves);
        let mut iter = moves.iter();
        assert_eq!(5, iter.len());
        assert_eq!(vec![0], iter.next().unwrap().1); // use die 1 to exit
        assert_eq!(vec![0], iter.next().unwrap().1); // use die 2 to exit
        assert_eq!(vec![0, 1], iter.next().unwrap().1); // use die 2 to exit then die 1 to move
        assert_eq!(vec![0, 1, 2, 3, 4, 5, 6], iter.next().unwrap().1); // use die 1 to exit then die 2 to move
        assert_eq!(vec![0, 1, 2, 3, 4, 5, 53], iter.next().unwrap().1); // use die 1 to exit then die 2 to move to center
    }

    #[test]
    fn test_center_exit_moves() {
        let dice = Dice::new(1, 4);
        let mut moves = BTreeSet::new();
        center_exit_rules(&dice, Entity::from_raw(12), &mut moves);
        let mut iter = moves.iter();
        assert_eq!(2, iter.len());
        assert_eq!(vec![41], iter.next().unwrap().1); // use die 1 to exit
        assert_eq!(vec![41, 42, 43, 44, 45], iter.next().unwrap().1); // use die 1 to exit then die 2 to move
    }

    #[test]
    fn test_basic_moves() {
        let dice = Dice::new(5, 5);
        let marble = Marble{ index: 43, prev_index: 42, origin: Vec3::ZERO };
        let mut moves = BTreeSet::new();
        basic_rules(&dice, Entity::from_raw(12), &marble, &mut moves);
        let mut iter = moves.iter();
        assert_eq!(2, moves.len());
        assert_eq!(vec![44, 45, 46, 47, 48], iter.next().unwrap().1);
        assert_eq!(vec![44, 45, 46, 47, 48], iter.next().unwrap().1);

        let dice = Dice::new(4, 1);
        let marble = Marble{ index: 52, prev_index: 52, origin: Vec3::ZERO };
        moves = BTreeSet::new();
        basic_rules(&dice, Entity::from_raw(13), &marble, &mut moves);
        assert_eq!(0, moves.len());
    }
}
