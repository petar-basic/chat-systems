use chrono::NaiveDate;
use serde_json::Value;
use uuid::Uuid;

use super::models::{ColumnKind, ColumnOptions, LINK_MAX_CHARS, TEXT_MAX_CHARS};

#[derive(Debug, PartialEq)]
pub enum Cell {
    Clear,
    Set(Value),
}

pub fn normalize(kind: ColumnKind, options: &ColumnOptions, value: &Value) -> Result<Cell, String> {
    if value.is_null() {
        return Ok(Cell::Clear);
    }
    match kind {
        ColumnKind::Text => {
            let text = value.as_str().ok_or("expects text")?;
            if text.trim().is_empty() {
                return Ok(Cell::Clear);
            }
            if text.chars().count() > TEXT_MAX_CHARS {
                return Err(format!("takes at most {TEXT_MAX_CHARS} characters"));
            }
            Ok(Cell::Set(Value::String(text.to_string())))
        }
        ColumnKind::Number => match value {
            Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => {
                Ok(Cell::Set(value.clone()))
            }
            _ => Err("expects a number".into()),
        },
        ColumnKind::Select => {
            let id = value.as_str().ok_or("expects one of its choices")?;
            let choice = options.choice(id).ok_or("has no such choice")?;
            Ok(Cell::Set(Value::String(choice.id.to_string())))
        }
        ColumnKind::MultiSelect => {
            let ids = value.as_array().ok_or("expects a list of its choices")?;
            let mut chosen = Vec::with_capacity(ids.len());
            for id in ids {
                let choice = id
                    .as_str()
                    .and_then(|id| options.choice(id))
                    .ok_or("has no such choice")?;
                if !chosen.contains(&choice.id) {
                    chosen.push(choice.id);
                }
            }
            if chosen.is_empty() {
                return Ok(Cell::Clear);
            }
            let ordered: Vec<Value> = options
                .choices
                .iter()
                .filter(|choice| chosen.contains(&choice.id))
                .map(|choice| Value::String(choice.id.to_string()))
                .collect();
            Ok(Cell::Set(Value::Array(ordered)))
        }
        ColumnKind::Person => {
            let id = value
                .as_str()
                .and_then(|id| id.parse::<Uuid>().ok())
                .ok_or("expects a person")?;
            Ok(Cell::Set(Value::String(id.to_string())))
        }
        ColumnKind::Date => {
            let date = value
                .as_str()
                .and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
                .ok_or("expects a date as YYYY-MM-DD")?;
            Ok(Cell::Set(Value::String(
                date.format("%Y-%m-%d").to_string(),
            )))
        }
        ColumnKind::Checkbox => match value {
            Value::Bool(_) => Ok(Cell::Set(value.clone())),
            _ => Err("expects true or false".into()),
        },
        ColumnKind::Link => {
            let link = value.as_str().ok_or("expects a link")?.trim();
            if link.is_empty() {
                return Ok(Cell::Clear);
            }
            if link.chars().count() > LINK_MAX_CHARS {
                return Err(format!(
                    "takes links of at most {LINK_MAX_CHARS} characters"
                ));
            }
            if !(link.starts_with("http://") || link.starts_with("https://"))
                || link.chars().any(|c| c.is_control() || c.is_whitespace())
            {
                return Err("expects a link starting with http:// or https://".into());
            }
            Ok(Cell::Set(Value::String(link.to_string())))
        }
    }
}

pub fn person_of(kind: ColumnKind, value: &Value) -> Option<Uuid> {
    match kind {
        ColumnKind::Person => value.as_str().and_then(|id| id.parse().ok()),
        _ => None,
    }
}

fn number_text(value: &Value) -> Option<String> {
    let n = value.as_f64()?;
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return Some(format!("{}", n as i64));
    }
    Some(n.to_string())
}

pub fn convert(
    from: (ColumnKind, &ColumnOptions),
    to: (ColumnKind, &ColumnOptions),
    value: &Value,
) -> Option<Value> {
    let carried = match (from.0, to.0) {
        (a, b) if a == b => value.clone(),
        (ColumnKind::Number, ColumnKind::Text) => Value::String(number_text(value)?),
        (ColumnKind::Text, ColumnKind::Number) => {
            let n = value.as_str()?.trim().parse::<f64>().ok()?;
            Value::Number(serde_json::Number::from_f64(n)?)
        }
        (ColumnKind::Select, ColumnKind::Text) => {
            Value::String(from.1.choice(value.as_str()?)?.label.clone())
        }
        (ColumnKind::Text, ColumnKind::Select) => {
            Value::String(to.1.choice_labelled(value.as_str()?)?.id.to_string())
        }
        _ => return None,
    };
    match normalize(to.0, to.1, &carried) {
        Ok(Cell::Set(value)) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lists::models::{Choice, ChoiceColor};
    use serde_json::json;

    fn choices() -> (ColumnOptions, Uuid, Uuid) {
        let (todo, done) = (Uuid::new_v4(), Uuid::new_v4());
        let options = ColumnOptions {
            choices: vec![
                Choice {
                    id: todo,
                    label: "To do".into(),
                    color: ChoiceColor::Gray,
                },
                Choice {
                    id: done,
                    label: "Done".into(),
                    color: ChoiceColor::Green,
                },
            ],
            number: None,
        };
        (options, todo, done)
    }

    fn set(kind: ColumnKind, options: &ColumnOptions, value: Value) -> Value {
        match normalize(kind, options, &value) {
            Ok(Cell::Set(v)) => v,
            other => panic!("{kind:?} {value} gave {other:?}"),
        }
    }

    fn refused(kind: ColumnKind, options: &ColumnOptions, value: Value) -> bool {
        normalize(kind, options, &value).is_err()
    }

    #[test]
    fn every_kind_takes_its_own_shape_and_refuses_others() {
        let none = ColumnOptions::default();
        let (options, todo, done) = choices();

        assert_eq!(set(ColumnKind::Text, &none, json!("hi")), json!("hi"));
        assert!(refused(ColumnKind::Text, &none, json!(3)));
        assert!(refused(
            ColumnKind::Text,
            &none,
            json!("x".repeat(TEXT_MAX_CHARS + 1))
        ));

        assert_eq!(set(ColumnKind::Number, &none, json!(2.5)), json!(2.5));
        assert!(refused(ColumnKind::Number, &none, json!("2.5")));

        assert_eq!(
            set(ColumnKind::Select, &options, json!(todo.to_string())),
            json!(todo.to_string())
        );
        assert!(refused(
            ColumnKind::Select,
            &options,
            json!(Uuid::new_v4().to_string())
        ));
        assert!(refused(ColumnKind::Select, &options, json!("To do")));

        assert_eq!(
            set(
                ColumnKind::MultiSelect,
                &options,
                json!([done.to_string(), todo.to_string(), done.to_string()])
            ),
            json!([todo.to_string(), done.to_string()])
        );
        assert!(refused(
            ColumnKind::MultiSelect,
            &options,
            json!(todo.to_string())
        ));

        let person = Uuid::new_v4();
        assert_eq!(
            set(ColumnKind::Person, &none, json!(person.to_string())),
            json!(person.to_string())
        );
        assert!(refused(ColumnKind::Person, &none, json!("ana")));

        assert_eq!(
            set(ColumnKind::Date, &none, json!("2026-10-23")),
            json!("2026-10-23")
        );
        assert!(refused(ColumnKind::Date, &none, json!("23.10.2026")));
        assert!(refused(ColumnKind::Date, &none, json!("2026-02-30")));

        assert_eq!(set(ColumnKind::Checkbox, &none, json!(false)), json!(false));
        assert!(refused(ColumnKind::Checkbox, &none, json!("yes")));

        assert_eq!(
            set(ColumnKind::Link, &none, json!(" https://x.dev/a ")),
            json!("https://x.dev/a")
        );
        assert!(refused(
            ColumnKind::Link,
            &none,
            json!("javascript:alert(1)")
        ));
        assert!(refused(ColumnKind::Link, &none, json!("https://x.dev/a b")));
    }

    #[test]
    fn null_and_blank_clear_a_cell() {
        let (options, _, _) = choices();
        for kind in [
            ColumnKind::Text,
            ColumnKind::Number,
            ColumnKind::Date,
            ColumnKind::Person,
        ] {
            assert_eq!(normalize(kind, &options, &Value::Null), Ok(Cell::Clear));
        }
        assert_eq!(
            normalize(ColumnKind::Text, &options, &json!("  ")),
            Ok(Cell::Clear)
        );
        assert_eq!(
            normalize(ColumnKind::Link, &options, &json!("")),
            Ok(Cell::Clear)
        );
        assert_eq!(
            normalize(ColumnKind::MultiSelect, &options, &json!([])),
            Ok(Cell::Clear)
        );
    }

    #[test]
    fn numbers_and_text_convert_both_ways() {
        let none = ColumnOptions::default();
        let text = (ColumnKind::Text, &none);
        let number = (ColumnKind::Number, &none);
        assert_eq!(convert(number, text, &json!(3.0)), Some(json!("3")));
        assert_eq!(convert(number, text, &json!(2.5)), Some(json!("2.5")));
        assert_eq!(convert(text, number, &json!(" 42 ")), Some(json!(42.0)));
        assert_eq!(convert(text, number, &json!("forty")), None);
    }

    #[test]
    fn selects_and_text_convert_through_the_label() {
        let none = ColumnOptions::default();
        let (options, todo, _) = choices();
        assert_eq!(
            convert(
                (ColumnKind::Select, &options),
                (ColumnKind::Text, &none),
                &json!(todo.to_string())
            ),
            Some(json!("To do"))
        );
        assert_eq!(
            convert(
                (ColumnKind::Text, &none),
                (ColumnKind::Select, &options),
                &json!("to DO")
            ),
            Some(json!(todo.to_string()))
        );
        assert_eq!(
            convert(
                (ColumnKind::Text, &none),
                (ColumnKind::Select, &options),
                &json!("Blocked")
            ),
            None
        );
    }

    #[test]
    fn a_removed_choice_and_an_unrelated_kind_clear_the_cell() {
        let none = ColumnOptions::default();
        let (options, todo, done) = choices();
        let fewer = ColumnOptions {
            choices: options
                .choices
                .iter()
                .filter(|c| c.id == done)
                .cloned()
                .collect(),
            number: None,
        };
        let select = (ColumnKind::Select, &options);
        assert_eq!(
            convert(
                select,
                (ColumnKind::Select, &fewer),
                &json!(todo.to_string())
            ),
            None
        );
        assert_eq!(
            convert(
                select,
                (ColumnKind::Select, &fewer),
                &json!(done.to_string())
            ),
            Some(json!(done.to_string()))
        );
        assert_eq!(
            convert(
                (ColumnKind::Date, &none),
                (ColumnKind::Number, &none),
                &json!("2026-10-23")
            ),
            None
        );
    }
}
