use std::collections::HashMap;

use serde_json::Value;
use uuid::Uuid;

use super::models::{ColumnKind, ListColumn, ListItem};

fn cell_text(column: &ListColumn, value: &Value, people: &HashMap<Uuid, String>) -> String {
    let label = |id: &Value| {
        id.as_str()
            .and_then(|id| column.options.choice(id))
            .map(|choice| choice.label.clone())
            .unwrap_or_default()
    };
    match (column.kind, value) {
        (_, Value::Null) => String::new(),
        (ColumnKind::Select, id) => label(id),
        (ColumnKind::MultiSelect, Value::Array(ids)) => {
            ids.iter().map(label).collect::<Vec<_>>().join("; ")
        }
        (ColumnKind::Person, Value::String(id)) => id
            .parse::<Uuid>()
            .ok()
            .and_then(|id| people.get(&id).cloned())
            .unwrap_or_else(|| id.clone()),
        (_, Value::String(text)) => text.clone(),
        (_, other) => other.to_string(),
    }
}

fn field(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

pub fn people_in(columns: &[ListColumn], items: &[ListItem]) -> Vec<Uuid> {
    let mut people: Vec<Uuid> = items
        .iter()
        .flat_map(|item| {
            columns
                .iter()
                .filter(|column| column.kind == ColumnKind::Person)
                .filter_map(|column| item.values.get(column.id.to_string()))
                .filter_map(|value| value.as_str().and_then(|id| id.parse().ok()))
        })
        .collect();
    people.sort();
    people.dedup();
    people
}

pub fn table(
    columns: &[ListColumn],
    items: &[ListItem],
    people: &HashMap<Uuid, String>,
) -> Vec<u8> {
    let mut out = String::new();
    let header: Vec<String> = columns.iter().map(|column| field(&column.name)).collect();
    out.push_str(&header.join(","));
    out.push_str("\r\n");
    for item in items {
        let row: Vec<String> = columns
            .iter()
            .map(|column| {
                let value = item
                    .values
                    .get(column.id.to_string())
                    .cloned()
                    .unwrap_or(Value::Null);
                field(&cell_text(column, &value, people))
            })
            .collect();
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lists::models::{Choice, ChoiceColor, ColumnOptions};
    use chrono::Utc;
    use serde_json::json;
    use sqlx::types::Json;

    fn column(name: &str, kind: ColumnKind, options: ColumnOptions) -> ListColumn {
        ListColumn {
            id: Uuid::new_v4(),
            list_id: Uuid::nil(),
            position: 0,
            name: name.into(),
            kind,
            options: Json(options),
        }
    }

    #[test]
    fn a_list_reads_as_a_spreadsheet_would() {
        let done = Uuid::new_v4();
        let ana = Uuid::new_v4();
        let status = column(
            "Status",
            ColumnKind::Select,
            ColumnOptions {
                choices: vec![Choice {
                    id: done,
                    label: "Done".into(),
                    color: ChoiceColor::Green,
                }],
                number: None,
            },
        );
        let title = column(
            "Title \"quoted\"",
            ColumnKind::Text,
            ColumnOptions::default(),
        );
        let owner = column("Owner", ColumnKind::Person, ColumnOptions::default());
        let points = column("Points", ColumnKind::Number, ColumnOptions::default());
        let item = ListItem {
            id: Uuid::new_v4(),
            list_id: Uuid::nil(),
            rank: "V".into(),
            values: json!({
                title.id.to_string(): "Ship it, finally",
                status.id.to_string(): done.to_string(),
                owner.id.to_string(): ana.to_string(),
                points.id.to_string(): 3,
            }),
            created_by: Uuid::nil(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let columns = [title, status, owner, points];
        assert_eq!(people_in(&columns, std::slice::from_ref(&item)), vec![ana]);
        let people = HashMap::from([(ana, "Ana".to_string())]);
        let text = String::from_utf8(table(&columns, &[item], &people)).unwrap();
        assert_eq!(
            text,
            "\"Title \"\"quoted\"\"\",\"Status\",\"Owner\",\"Points\"\r\n\
             \"Ship it, finally\",\"Done\",\"Ana\",\"3\"\r\n"
        );
    }
}
