use super::{parse, Kind, Ruleset};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

struct RulesetVisitor {}

impl RulesetVisitor {
    fn visit_str<E>(self, v: &str) -> Result<Ruleset, E>
    where
        E: serde::de::Error,
    {
        if let Some(rules) = parse(v) {
            Ok(Ruleset { rules })
        } else {
            Err(E::custom("invalid ruleset"))
        }
    }
}

impl<'de> Visitor<'de> for RulesetVisitor {
    type Value = Ruleset;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid ruleset")
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(v)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(v)
    }
}

impl<'de> Deserialize<'de> for Ruleset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RulesetVisitor {})
    }
}

impl Serialize for Ruleset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let mut s = String::new();
        for rule in &self.rules {
            for case in &rule.cases {
                if case.negated {
                    s.push('!');
                }

                match &case.kind {
                    Kind::RegexUnparsed => panic!("unparsed regex"),
                    Kind::Regex(lazy) => {
                        s.push_str(&format!("r {}", lazy.as_str()));
                    }
                };

                s.push('\n');
            }

            s.push_str("or\n");
        }

        // Remove the final next
        // This way is funnier than the better solution
        s.pop();
        s.pop();
        s.pop();

        serializer.serialize_str(&s)
    }
}
