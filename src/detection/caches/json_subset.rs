//! A streaming JSON reader that keeps only what the matchers read.
//!
//! A manifest is mostly data nobody looks at (scripts, metadata, a lockfile's
//! integrity hashes). Building a `serde_json::Value` for all of it costs
//! tens of bytes per node, so a hostile 30 MB file of `[0,0,0,…]` needs
//! hundreds of MB. This reader walks the text once: top-level keys outside
//! `MANIFEST_KEYS` are skipped without allocating, and the kept ones are
//! built under a node budget, so memory is bounded whatever the file holds.

use serde::de::{DeserializeSeed, Deserializer, Error, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::cell::Cell;
use std::fmt;

/// Top-level keys some matcher reads. Add a key here when a matcher starts
/// reading a new one, or it will silently see nothing.
const MANIFEST_KEYS: [&str; 8] = [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
    "require",
    "require-dev",
    "imports",
    "packages",
];

/// Nodes (values, array elements, object members) built from the kept keys
/// before the file is declared unparseable. A large `package-lock.json` has
/// tens of thousands; this keeps the worst case in the tens of megabytes.
pub(crate) const MAX_JSON_NODES: usize = 500_000;

/// The kept top-level keys of the JSON document in `text`, as an object
pub(crate) fn parse_manifest(text: &str) -> Result<Value, serde_json::Error> {
    read_manifest(&mut serde_json::Deserializer::from_str(text))
}

fn read_manifest<'de, R: serde_json::de::Read<'de>>(
    de: &mut serde_json::Deserializer<R>,
) -> Result<Value, serde_json::Error> {
    let budget = Cell::new(MAX_JSON_NODES);
    let value = Top { budget: &budget }.deserialize(&mut *de)?;
    de.end()?;
    Ok(value)
}

/// The document root: keeps `MANIFEST_KEYS` members of an object, and
/// reduces anything else to `Null` (no matcher reads a non-object manifest)
struct Top<'a> {
    budget: &'a Cell<usize>,
}

impl<'de> DeserializeSeed<'de> for Top<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Top<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut kept = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if MANIFEST_KEYS.contains(&key.as_str()) {
                let value = map.next_value_seed(Bounded {
                    budget: self.budget,
                })?;
                kept.insert(key, value);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Value::Object(kept))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Value::Null)
    }

    fn visit_bool<E: Error>(self, _: bool) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: Error>(self, _: i64) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_u64<E: Error>(self, _: u64) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_f64<E: Error>(self, _: f64) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: Error>(self, _: &str) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
}

/// A full `Value`, but every node spends from a shared budget
struct Bounded<'a> {
    budget: &'a Cell<usize>,
}

impl Bounded<'_> {
    fn spend<E: Error>(&self) -> Result<(), E> {
        match self.budget.get() {
            0 => Err(E::custom("JSON value has too many nodes")),
            left => {
                self.budget.set(left - 1);
                Ok(())
            }
        }
    }
}

impl<'de> DeserializeSeed<'de> for Bounded<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Bounded<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: Error>(self, v: bool) -> Result<Value, E> {
        self.spend()?;
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: Error>(self, v: i64) -> Result<Value, E> {
        self.spend()?;
        Ok(Value::Number(Number::from(v)))
    }
    fn visit_u64<E: Error>(self, v: u64) -> Result<Value, E> {
        self.spend()?;
        Ok(Value::Number(Number::from(v)))
    }
    fn visit_f64<E: Error>(self, v: f64) -> Result<Value, E> {
        self.spend()?;
        Ok(Number::from_f64(v).map_or(Value::Null, Value::Number))
    }
    fn visit_str<E: Error>(self, v: &str) -> Result<Value, E> {
        self.spend()?;
        Ok(Value::String(v.to_owned()))
    }
    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        self.spend()?;
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        self.spend()?;
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Bounded {
            budget: self.budget,
        })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        self.spend()?;
        let mut members = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.spend()?;
            let value = map.next_value_seed(Bounded {
                budget: self.budget,
            })?;
            members.insert(key, value);
        }
        Ok(Value::Object(members))
    }
}
