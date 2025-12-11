use crate::state::{Event, Order, UserAccount};
use solana_program::pubkey::Pubkey;

/// Simple in-memory matching routine between an incoming taker order
/// and an array of maker orders on the opposite side.
pub fn match_orders(
    taker: &mut UserAccount,
    makers: &mut [UserAccount],
    mut remaining_base_lots: i64,
    max_quote_change: &mut i64,
    side_is_bid: bool,
    events: &mut Vec<Event>,
) {
    for maker in makers.iter_mut() {
        if remaining_base_lots <= 0 {
            break;
        }

        for order in maker.open_orders.iter_mut() {
            if !order.is_active || order.side_is_bid == side_is_bid {
                continue;
            }

            let trade_base = remaining_base_lots.min(order.base_lots);
            if trade_base <= 0 {
                continue;
            }

            let quote_change = trade_base * order.price_lots;

            if side_is_bid {
                taker.base_position += trade_base;
                taker.quote_position -= quote_change;
                maker.base_position -= trade_base;
                maker.quote_position += quote_change;
            } else {
                taker.base_position -= trade_base;
                taker.quote_position += quote_change;
                maker.base_position += trade_base;
                maker.quote_position -= quote_change;
            }

            order.base_lots -= trade_base;
            if order.base_lots == 0 {
                order.is_active = false;
            }

            *max_quote_change += quote_change.abs();
            remaining_base_lots -= trade_base;

            events.push(Event::Trade {
                maker: maker.owner,
                taker: taker.owner,
                price_lots: order.price_lots,
                base_lots: trade_base,
            });
        }
    }
}

pub fn find_user<'a>(users: &'a mut [UserAccount], owner: &Pubkey) -> Option<&'a mut UserAccount> {
    users.iter_mut().find(|u| &u.owner == owner)
}
