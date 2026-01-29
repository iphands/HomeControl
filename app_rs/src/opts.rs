use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptValue {
    #[serde(rename = "val")]
    pub value: serde_json::Value,
    #[serde(rename = "type")]
    pub type_name: String,
}

impl OptValue {
    pub fn create_int(val: i64) -> Self {
        Self {
            value: serde_json::Value::Number(val.into()),
            type_name: "int".to_string(),
        }
    }

    pub fn create_float(val: f64) -> Self {
        Self {
            value: serde_json::Value::Number(serde_json::Number::from_f64(val).unwrap()),
            type_name: "float".to_string(),
        }
    }

    pub fn create_bool(val: bool) -> Self {
        Self {
            value: serde_json::Value::Bool(val),
            type_name: "bool".to_string(),
        }
    }

    pub fn create_color(color: [u8; 3]) -> Self {
        Self {
            value: serde_json::Value::Array(vec![
                serde_json::Value::Number(color[0].into()),
                serde_json::Value::Number(color[1].into()),
                serde_json::Value::Number(color[2].into()),
            ]),
            type_name: "color".to_string(),
        }
    }
}

pub fn parse_color(val: &serde_json::Value) -> Option<[u8; 3]> {
    match val {
        serde_json::Value::String(s) => {
            if s.starts_with('#') && s.len() == 7 {
                // Parse hex color #RRGGBB
                let r = u8::from_str_radix(&s[1..3], 16).ok()?;
                let g = u8::from_str_radix(&s[3..5], 16).ok()?;
                let b = u8::from_str_radix(&s[5..7], 16).ok()?;
                Some([r, g, b])
            } else {
                None
            }
        }
        serde_json::Value::Array(arr) if arr.len() == 3 => {
            let r = arr[0].as_u64()? as u8;
            let g = arr[1].as_u64()? as u8;
            let b = arr[2].as_u64()? as u8;
            Some([r, g, b])
        }
        _ => None,
    }
}
