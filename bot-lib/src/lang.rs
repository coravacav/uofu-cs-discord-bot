mod case;
mod rule;
pub mod ruleset;

#[macro_export]
macro_rules! fast_ruleset {
    ($($x:expr),*) => {{
        $crate::lang::ruleset::Ruleset::parse(&[$($x),*].join("\n")).unwrap()
    }};
}

#[cfg(test)]
mod test {
    #[test]
    fn test_detection() {
        let ruleset = fast_ruleset!("r 1234", "or", "r :3", "or", "r mew");

        assert!(ruleset.matches("mew"));
        assert!(ruleset.matches(":3"));
        assert!(ruleset.matches("1234"));
        assert!(!ruleset.matches("123"));
    }

    #[test]
    fn test_detection_2() {
        let ruleset = fast_ruleset!(
            r"r (?i)\bme+o*w\b",
            "or",
            "r (?i)[ou]w[ou]",
            "or",
            "r 喵",
            "or",
            "r :3",
            "or",
            r"r (?i)\bee+p.*",
            "or",
            "r (?i)ny+a+",
            "or",
            "r (?i)mrr+[pb]",
            "or",
            "r (?i)pu+rr+"
        );

        assert!(ruleset.matches("mew"));
        assert!(ruleset.matches(":3"));
        assert!(!ruleset.matches("123"));
        assert!(ruleset.matches("meow"));
    }
}
