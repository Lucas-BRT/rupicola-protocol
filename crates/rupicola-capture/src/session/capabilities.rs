use serde::Serialize;

/// One thing a backend may or may not be able to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Performs every [`rupicola_protocol::UiAction`], including those that name a widget.
    Actions,
    /// Performs the actions that only need screen coordinates: `click-at`, `scroll`
    /// without a target, and `wait-ms`.
    Pointer,
    /// Reports a UI tree with each screenshot.
    UiTree,
    /// Records walkthrough clips.
    Video,
    /// Honours the environment's color scheme.
    ColorScheme,
    /// Honours the environment's scale.
    Scale,
}

impl Capability {
    const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Name used in messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Actions => "actions",
            Self::Pointer => "pointer",
            Self::UiTree => "ui-tree",
            Self::Video => "video",
            Self::ColorScheme => "color-scheme",
            Self::Scale => "scale",
        }
    }

    pub(crate) const ALL: [Self; 6] = [
        Self::Actions,
        Self::Pointer,
        Self::UiTree,
        Self::Video,
        Self::ColorScheme,
        Self::Scale,
    ];
}

/// What a backend can do, or what a plan needs; checked before anything starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(u8);

impl Capabilities {
    pub(super) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// A set holding exactly `items`.
    #[must_use]
    pub const fn of(items: &[Capability]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < items.len() {
            bits |= items[index].bit();
            index += 1;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, item: Capability) -> bool {
        self.0 & item.bit() != 0
    }

    /// `self` with `item` added.
    #[must_use]
    pub const fn with(self, item: Capability) -> Self {
        Self(self.0 | item.bit())
    }

    /// The capabilities in `needed` that `self` lacks.
    #[must_use]
    pub fn missing(self, needed: Self) -> Vec<Capability> {
        // Full actions include the coordinate-only ones.
        let has = |item: Capability| {
            self.contains(item)
                || (item == Capability::Pointer && self.contains(Capability::Actions))
        };
        Capability::ALL
            .into_iter()
            .filter(|item| needed.contains(*item) && !has(*item))
            .collect()
    }
}

impl Serialize for Capabilities {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            Capability::ALL
                .into_iter()
                .filter(|item| self.contains(*item)),
        )
    }
}
