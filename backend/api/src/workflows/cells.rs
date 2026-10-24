use chrono::NaiveDate;
use serde_json::Value;
use uuid::Uuid;

use super::models::{placeholders, VariableKind};
use crate::lists::models::{ColumnKind, ListColumn};

pub fn single_variable(template: &str) -> Option<&str> {
    let inner = template.trim().strip_prefix("{{")?.strip_suffix("}}")?;
    (!inner.contains("{{") && !inner.contains("}}")).then(|| inner.trim())
}

fn person(text: &str) -> Option<Uuid> {
    let text = text.trim();
    if let Ok(id) = text.parse() {
        return Some(id);
    }
    let rest = text.strip_prefix("@[")?;
    let open = rest.rfind("](")?;
    rest[open + 2..].strip_suffix(')')?.parse().ok()
}

fn choice(column: &ListColumn, text: &str) -> Option<Uuid> {
    column
        .options
        .choice(text.trim())
        .or_else(|| column.options.choice_labelled(text))
        .map(|choice| choice.id)
}

fn checked(text: &str) -> Option<bool> {
    match text.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" | "checked" | "done" => Some(true),
        "false" | "no" | "0" | "unchecked" => Some(false),
        _ => None,
    }
}

pub fn value(column: &ListColumn, rendered: &str) -> Result<Option<Value>, String> {
    let text = rendered.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let name = column.name.as_str();
    let value = match column.kind {
        ColumnKind::Text | ColumnKind::Link => Value::String(text.to_string()),
        ColumnKind::Number => text
            .replace(',', "")
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| {
                format!("\u{201c}{name}\u{201d} needs a number, not \u{201c}{text}\u{201d}")
            })?,
        ColumnKind::Select => choice(column, text)
            .map(|id| Value::String(id.to_string()))
            .ok_or_else(|| {
                format!("\u{201c}{name}\u{201d} has no choice \u{201c}{text}\u{201d}")
            })?,
        ColumnKind::MultiSelect => {
            let mut ids = Vec::new();
            for part in text
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                let id = choice(column, part).ok_or_else(|| {
                    format!("\u{201c}{name}\u{201d} has no choice \u{201c}{part}\u{201d}")
                })?;
                ids.push(Value::String(id.to_string()));
            }
            Value::Array(ids)
        }
        ColumnKind::Person => person(text)
            .map(|id| Value::String(id.to_string()))
            .ok_or_else(|| format!("\u{201c}{name}\u{201d} needs a person"))?,
        ColumnKind::Date => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(|date| Value::String(date.format("%Y-%m-%d").to_string()))
            .map_err(|_| format!("\u{201c}{name}\u{201d} needs a date as YYYY-MM-DD"))?,
        ColumnKind::Checkbox => checked(text)
            .map(Value::Bool)
            .ok_or_else(|| format!("\u{201c}{name}\u{201d} needs yes or no"))?,
    };
    Ok(Some(value))
}

pub fn check<'a>(
    column: &ListColumn,
    template: &str,
    kind_of: impl Fn(&str) -> VariableKind<'a>,
) -> Result<(), String> {
    let name = column.name.as_str();
    if placeholders(template).is_empty() {
        return value(column, template).map(|_| ());
    }
    if matches!(column.kind, ColumnKind::Text | ColumnKind::Link) {
        return Ok(());
    }
    let Some(variable) = single_variable(template) else {
        return Err(format!(
            "\u{201c}{name}\u{201d} takes a single value or variable"
        ));
    };
    let fits = match (column.kind, kind_of(variable)) {
        (_, VariableKind::Text) => true,
        (ColumnKind::Person, VariableKind::Person) => true,
        (ColumnKind::Date, VariableKind::Date) => true,
        (ColumnKind::Select | ColumnKind::MultiSelect, VariableKind::Choice(options)) => {
            if let Some(missing) = options
                .iter()
                .find(|option| choice(column, option).is_none())
            {
                return Err(format!(
                    "\u{201c}{name}\u{201d} has no choice \u{201c}{missing}\u{201d}, which {{{{{variable}}}}} can answer"
                ));
            }
            true
        }
        _ => false,
    };
    if fits {
        Ok(())
    } else {
        Err(format!(
            "\u{201c}{name}\u{201d} can't take {{{{{variable}}}}}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lists::models::{Choice, ChoiceColor, ColumnOptions};
    use serde_json::json;
    use sqlx::types::Json;

    fn column(kind: ColumnKind, choices: &[&str]) -> ListColumn {
        ListColumn {
            id: Uuid::nil(),
            list_id: Uuid::nil(),
            position: 0,
            name: "Col".into(),
            kind,
            options: Json(ColumnOptions {
                choices: choices
                    .iter()
                    .map(|label| Choice {
                        id: Uuid::new_v4(),
                        label: label.to_string(),
                        color: ChoiceColor::Gray,
                    })
                    .collect(),
                number: None,
            }),
            hidden: false,
            due: false,
        }
    }

    #[test]
    fn a_rendered_value_becomes_the_cell_its_column_holds() {
        let person = Uuid::new_v4();
        assert_eq!(
            value(
                &column(ColumnKind::Person, &[]),
                &format!("@[Ana]({person})")
            )
            .unwrap(),
            Some(json!(person.to_string()))
        );
        assert_eq!(
            value(&column(ColumnKind::Person, &[]), &person.to_string()).unwrap(),
            Some(json!(person.to_string()))
        );
        assert_eq!(
            value(&column(ColumnKind::Number, &[]), " 1,250.5 ").unwrap(),
            Some(json!(1250.5))
        );
        assert_eq!(
            value(&column(ColumnKind::Checkbox, &[]), "Yes").unwrap(),
            Some(json!(true))
        );
        assert_eq!(
            value(&column(ColumnKind::Date, &[]), "2026-11-02").unwrap(),
            Some(json!("2026-11-02"))
        );
        assert_eq!(value(&column(ColumnKind::Text, &[]), "  ").unwrap(), None);
        let status = column(ColumnKind::Select, &["Open", "Done"]);
        let done = status.options.choices[1].id.to_string();
        assert_eq!(value(&status, "done").unwrap(), Some(json!(done)));
        assert_eq!(
            value(&status, "Later").unwrap_err(),
            "\u{201c}Col\u{201d} has no choice \u{201c}Later\u{201d}"
        );
        assert_eq!(
            value(&column(ColumnKind::Number, &[]), "lots").unwrap_err(),
            "\u{201c}Col\u{201d} needs a number, not \u{201c}lots\u{201d}"
        );
    }

    #[test]
    fn a_column_takes_a_fixed_value_or_a_variable_of_its_kind() {
        let kinds = |name: &str| match name {
            "trigger.user" => VariableKind::Person,
            "form.from" => VariableKind::Date,
            _ => VariableKind::Text,
        };
        let person = column(ColumnKind::Person, &[]);
        assert!(check(&person, "{{trigger.user}}", kinds).is_ok());
        assert!(check(&person, "{{ webhook.owner }}", kinds).is_ok());
        assert_eq!(
            check(&person, "{{form.from}}", kinds).unwrap_err(),
            "\u{201c}Col\u{201d} can't take {{form.from}}"
        );
        assert_eq!(
            check(&person, "hi {{trigger.user}}", kinds).unwrap_err(),
            "\u{201c}Col\u{201d} takes a single value or variable"
        );
        assert!(check(&person, "nobody", kinds).is_err());
        assert!(check(
            &column(ColumnKind::Text, &[]),
            "{{trigger.user}} on {{form.from}}",
            kinds
        )
        .is_ok());
        assert!(check(&column(ColumnKind::Date, &[]), "{{form.from}}", kinds).is_ok());

        let status = column(ColumnKind::Select, &["Sick", "Holiday"]);
        let options = ["Sick".to_string(), "Holiday".to_string()];
        let more = ["Sick".to_string(), "Remote".to_string()];
        assert!(check(&status, "{{form.kind}}", |_| VariableKind::Choice(&options)).is_ok());
        assert_eq!(
            check(&status, "{{form.kind}}", |_| VariableKind::Choice(&more)).unwrap_err(),
            "\u{201c}Col\u{201d} has no choice \u{201c}Remote\u{201d}, which {{form.kind}} can answer"
        );
        assert!(check(&status, "holiday", kinds).is_ok());
    }
}
