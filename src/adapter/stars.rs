//! Telegram Stars API: transactions, refunds.

use grammers_client::tl;

use super::GrammersAdapter;
use crate::error::ApiError;
use crate::types::*;

impl GrammersAdapter {
    pub(crate) async fn impl_get_star_transactions(
        &self,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        let result = self
            .client
            .invoke(&tl::functions::payments::GetStarsTransactions {
                inbound: false,
                outbound: false,
                ascending: false,
                ton: false,
                subscription_id: None,
                peer: tl::types::InputPeerSelf {}.into(),
                offset: offset.unwrap_or("").to_string(),
                limit: limit.unwrap_or(100),
            })
            .await
            .map_err(Self::convert_error)?;

        let tl::enums::payments::StarsStatus::Status(status) = result;
        let tl::enums::StarsAmount::Amount(balance_amount) = status.balance else {
            return Ok(StarTransactions {
                balance: StarBalance {
                    amount: 0,
                    nanos: 0,
                },
                transactions: vec![],
                next_offset: None,
            });
        };

        let transactions = status
            .history
            .unwrap_or_default()
            .into_iter()
            .map(|tx| {
                let tl::enums::StarsTransaction::Transaction(t) = tx;
                let (amount, nanos) = match t.amount {
                    tl::enums::StarsAmount::Amount(a) => (a.amount, a.nanos),
                    _ => (0, 0),
                };
                let source = match t.peer {
                    tl::enums::StarsTransactionPeer::Peer(p) => match p.peer {
                        tl::enums::Peer::User(u) => {
                            StarTransactionPeer::User(UserId(u.user_id as u64))
                        }
                        _ => StarTransactionPeer::Unknown,
                    },
                    tl::enums::StarsTransactionPeer::AppStore => StarTransactionPeer::AppStore,
                    tl::enums::StarsTransactionPeer::PlayMarket => StarTransactionPeer::PlayMarket,
                    tl::enums::StarsTransactionPeer::Fragment => StarTransactionPeer::Fragment,
                    tl::enums::StarsTransactionPeer::PremiumBot => StarTransactionPeer::PremiumBot,
                    tl::enums::StarsTransactionPeer::Ads => StarTransactionPeer::Ads,
                    tl::enums::StarsTransactionPeer::Api => StarTransactionPeer::Api,
                    _ => StarTransactionPeer::Unknown,
                };
                StarTransaction {
                    id: t.id,
                    amount,
                    nanos,
                    date: t.date,
                    source,
                    title: t.title,
                    description: t.description,
                    is_refund: t.refund,
                }
            })
            .collect();

        Ok(StarTransactions {
            balance: StarBalance {
                amount: balance_amount.amount,
                nanos: balance_amount.nanos,
            },
            transactions,
            next_offset: status.next_offset,
        })
    }

    pub(crate) async fn impl_refund_star_payment(
        &self,
        user_id: UserId,
        charge_id: &str,
    ) -> Result<(), ApiError> {
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        self.client
            .invoke(&tl::functions::payments::RefundStarsCharge {
                user_id: input_user,
                charge_id: charge_id.to_string(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }
}
