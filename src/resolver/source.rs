/// Provenance: which layer (or sentinel) supplied a resolved value.
///
/// Shared across scalar config resolution (`Resolved`) and skills resolution.
/// `Default` is the sentinel for fields with no contributing layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    User,
    Project,
    Local,
    School,
    Override,
    Default,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::User => "user",
            Source::Project => "project",
            Source::Local => "local",
            Source::School => "school",
            Source::Override => "override",
            Source::Default => "default",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: T,
    pub from: Source,
}

impl<T> Sourced<T> {
    pub fn new(value: T, from: Source) -> Self {
        Self { value, from }
    }

    pub fn at_default(value: T) -> Self {
        Self {
            value,
            from: Source::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_covers_every_variant() {
        for (s, expected) in [
            (Source::User, "user"),
            (Source::Project, "project"),
            (Source::Local, "local"),
            (Source::School, "school"),
            (Source::Override, "override"),
            (Source::Default, "default"),
        ] {
            assert_eq!(s.label(), expected);
        }
    }

    #[test]
    fn sourced_carries_value_and_origin() {
        let s = Sourced::new(42, Source::Project);
        assert_eq!(s.value, 42);
        assert_eq!(s.from, Source::Project);
    }

    #[test]
    fn sourced_at_default_helper() {
        let s: Sourced<&str> = Sourced::at_default("hi");
        assert_eq!(s.from, Source::Default);
        assert_eq!(s.value, "hi");
    }
}
