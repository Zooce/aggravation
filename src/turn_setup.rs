use bevy::prelude::*;
use crate::components::*;
use crate::constants::*;
use crate::shared_systems::HighlightEvent;
use crate::resources::*;
use std::collections::BTreeSet;

pub fn calc_possible_moves(
    dice_data: Res<DiceData>,
    current_player_marbles: Query<(Entity, &Marble), With<CurrentPlayer>>,
    opponent_marbles: Query<(&Marble, &Player, Option<&Evading>), Without<CurrentPlayer>>,
    mut current_player_data: ResMut<CurrentPlayerData>,
    game_data: Res<GameData>,
) {
    let player_data = game_data.players.get(&current_player_data.player).unwrap();
    let mut possible_moves = BTreeSet::new(); // so we disregard duplicates

    if player_data.power_up_status.home_run {
        let open_home_indexes: Vec<usize> = (FIRST_HOME_INDEX..=LAST_HOME_INDEX)
            .filter(|i| current_player_marbles.iter().find(|(_, m)| m.index == *i).is_none() )
            .collect();
        current_player_marbles.iter()
            // home runs are only for marbles that are not already home
            .filter(|(_, m)| !(FIRST_HOME_INDEX..=LAST_HOME_INDEX).contains(&m.index))
            // add each open home index as a possible move
            .for_each(|(e, _)| open_home_indexes.iter().for_each(|&i| {
                possible_moves.insert((e, vec![i], WhichDie::Neither));
            }));
        current_player_data.possible_moves = possible_moves.into_iter().map(|(entity, path, which)| {
            (entity, (*path.last().unwrap(), path.len(), which).into())
        }).collect();
        return;
    }

    if player_data.power_up_status.capture_nearest {
        current_player_marbles.iter()
            // cannot capture from the base or home
            .filter(|(_, m)| m.index != BOARD.len() && !(FIRST_HOME_INDEX..=LAST_HOME_INDEX).contains(&m.index))
            .for_each(|(e, m)| {
                let closest = opponent_marbles.iter()
                    .filter(|(om, _, ev)| {
                        ev.is_none() && // can't capture evading marbles
                        om.index != BOARD.len() && // can't capture marbles in the base
                        !(FIRST_HOME_INDEX..=LAST_HOME_INDEX).contains(&om.index) // can't capture marbles in the home row
                    })
                    // map all to shifted indexes
                    .map(|(om, op, _)| Player::shift_index(om.index, *op, current_player_data.player))
                    // we can only capture marbles in front of us
                    .filter(|i| i > &m.index)
                    // we can only capture in the center if we can enter the center
                    .filter(|i| *i != CENTER_INDEX || m.index <= 29)
                    // find the smallest distance between this marble and the opponent marbles
                    .min_by_key(|index| { // FIXME: we're not considering the possibility of more than one nearest capture
                        match *index {
                            // distance to center index depends on where the next entrance is
                            CENTER_INDEX => match m.index {
                                18..=29 => 29 - m.index + 1,
                                6..=17 => 17 - m.index + 1,
                                0..=5 => 5 - m.index + 1,
                                _ => usize::MAX,
                            }
                            idx => match m.index {
                                CENTER_INDEX => match idx {
                                    41..=47 => 47 - idx + 1,
                                    _ => usize::MAX,
                                }
                                _ => idx - m.index,
                            }
                        }
                    });
                if let Some(index) = closest {
                    let path = match index {
                        CENTER_INDEX => {
                            let mut path: Vec<_> = match m.index {
                                18..=29 => (m.index..=29).collect(),
                                6..=17 => (m.index..=17).collect(),
                                0..=5 => (m.index..=5).collect(),
                                _ => unreachable!(),
                            };
                            path.remove(0); // paths should not contain the current marble location
                            path.push(CENTER_INDEX);
                            path
                        }
                        i => match m.index {
                            CENTER_INDEX => (41..=i).collect(),
                            _ => (m.index + 1..=i).collect(),
                        }
                    };
                    possible_moves.insert((e, path, WhichDie::Neither));
                }
            });
    }

    if !dice_data.dice.is_empty() {
        for (entity, marble) in &current_player_marbles {
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
            let self_jump_violations = current_player_marbles.iter()
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
                });
            let evading_violations = opponent_marbles.iter()
                .filter(|(_, _, ev)| ev.is_some())
                .find(|(m, p, _)| {
                    Player::is_same_index(current_player_data.player, *path.last().unwrap(), **p, m.index)
                });
            if evading_violations.is_some() {
                println!("evading violation: {:?}", (entity, *path.last().unwrap(), which));
            }
            if self_jump_violations.is_none() && evading_violations.is_none() {
                Some((entity, (*path.last().unwrap(), path.len(), which).into()))
            } else {
                None
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
    mut next_state: ResMut<NextState<GameState>>,
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
        next_state.set(GameState::HumanTurn);
    } else {
        next_state.set(GameState::ComputerTurn);
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

    // TODO: test for capture nearest bug (unreachable code when using capture nearest after tile 29 with an opponent in the center)
}
