use std::time::Duration;

use barnacle_catalog::ShipIndex;
use barnacle_catalog::names::clean_answer;

use crate::draw::Draw;
use crate::ids::Snowflake;
use crate::ids::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub before_hint: Duration,
    pub after_hint: Duration,
}

impl Timing {
    pub const STANDARD: Self = Self {
        before_hint: Duration::from_secs(20),
        after_hint: Duration::from_secs(10),
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guess<'a> {
    pub author: UserId,
    pub author_is_bot: bool,
    pub message: Snowflake,
    pub text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solve {
    pub winner: UserId,
    pub ship: ShipIndex,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Round {
    draw: Draw,
    invoker: UserId,
    posted: Snowflake,
}

impl Draw {
    pub fn start(self, invoker: UserId, posted: Snowflake) -> Round {
        Round {
            draw: self,
            invoker,
            posted,
        }
    }
}

impl Round {
    pub fn draw(&self) -> &Draw {
        &self.draw
    }

    pub fn invoker(&self) -> UserId {
        self.invoker
    }

    pub fn posted(&self) -> Snowflake {
        self.posted
    }

    pub fn judge(&self, guess: &Guess<'_>) -> Option<Solve> {
        let correct = guess.message > self.posted
            && !guess.author_is_bot
            && self.draw.answers.contains(&clean_answer(guess.text));
        correct.then(|| Solve {
            winner: guess.author,
            ship: self.draw.ship().clone(),
            elapsed: self.posted.elapsed_until(guess.message),
        })
    }

    pub fn may_cancel(&self, user: UserId, can_manage_messages: bool) -> bool {
        user == self.invoker || can_manage_messages
    }
}
