mod case;
mod rule;
pub mod ruleset;
pub mod ruleset_combinator;

#[macro_export]
macro_rules! fast_ruleset {
    ($($x:expr_2021),*) => {{
        let ruleset: $crate::lang::ruleset::Ruleset = $crate::lang::ruleset::UnparsedRuleset::parse(&[$($x),*].join("\n")).unwrap().try_into().unwrap();
        ruleset
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

    #[test]
    fn no_luck() {
        let ruleset = fast_ruleset!(
            r"r (?i):.*k+o+p+t+a+.*:",
            r"!r <:kopta_1:1166893677617090642><:kopta_2:1166893728619831397><:kopta_3:1166893843283710052><:kopta_4:1166893878910124032>"
        );

        assert!(!ruleset.matches("luck"));
    }

    #[test]
    fn alc() {
        let ruleset = fast_ruleset!(
            r"r (?i)\balc(?:ohol(?:ism)?)?",
            r"or",
            r"r (?i)beer",
            r"or",
            r"r (?i)whiskey",
            r"or",
            r"r (?i)mezcal",
            r"or",
            r"r (?i)tequila",
            r"or",
            r"r (?i)soju"
        );

        assert!(ruleset.matches("alcohol"));
        assert!(ruleset.matches("beer"));
        assert!(ruleset.matches("whiskey"));
        assert!(ruleset.matches("mezcal"));
        assert!(ruleset.matches("tequila"));
        assert!(ruleset.matches("soju"));
        assert!(ruleset.matches("alc"));
    }

    #[test]
    fn arch() {
        let ruleset = fast_ruleset!(r"r (?i)\barch");

        assert!(ruleset.matches("arch"));
    }

    #[test]
    fn not_me() {
        let ruleset = fast_ruleset!(r"r <@216767618923757568>");

        assert!(!ruleset.matches("rust"));
    }
}
