use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerEngineInfo {
    pub engine: String, // "podman" | "docker" | "none"
    pub version: String,
    pub rootless: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPort {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_port: Option<u16>,
    pub container_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerItem {
    pub id: String,
    pub names: Vec<String>,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: serde_json::Value,
    pub ports: Vec<ContainerPort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerActionBody {
    pub server_id: i64,
    pub container_id: String,
    pub action: String, // "start" | "stop" | "restart" | "pause" | "unpause" | "delete"
    #[serde(default)]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogsOut {
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageItem {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePullBody {
    pub server_id: i64,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRunBody {
    pub server_id: i64,
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
}

pub fn parse_port_entry(val: &serde_json::Value) -> Vec<ContainerPort> {
    let mut results = Vec::new();

    if let Some(arr) = val.as_array() {
        for item in arr {
            results.extend(parse_port_entry(item));
        }
        return results;
    }

    if let Some(obj) = val.as_object() {
        let host_ip = obj
            .get("hostIP")
            .or_else(|| obj.get("host_ip"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let host_port = obj
            .get("hostPort")
            .or_else(|| obj.get("host_port"))
            .and_then(|v| v.as_u64())
            .map(|n| n as u16);
        let container_port = obj
            .get("containerPort")
            .or_else(|| obj.get("container_port"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u16;
        let protocol = obj
            .get("protocol")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        results.push(ContainerPort {
            host_ip,
            host_port,
            container_port,
            protocol,
        });
        return results;
    }

    if let Some(s) = val.as_str() {
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if part.contains("->") {
                let mut split = part.splitn(2, "->");
                let host_part = split.next().unwrap_or("");
                let cont_part = split.next().unwrap_or("");

                let (c_port_str, proto) = if cont_part.contains('/') {
                    let mut cp_split = cont_part.splitn(2, '/');
                    (cp_split.next().unwrap_or(""), cp_split.next().unwrap_or("tcp"))
                } else {
                    (cont_part, "tcp")
                };

                let mut host_ip = None;
                let mut host_port = None;
                if host_part.contains(':') {
                    if let Some(idx) = host_part.rfind(':') {
                        host_ip = Some(host_part[..idx].to_string());
                        host_port = host_part[idx + 1..].parse::<u16>().ok();
                    }
                } else {
                    host_port = host_part.parse::<u16>().ok();
                }

                results.push(ContainerPort {
                    host_ip,
                    host_port,
                    container_port: c_port_str.parse::<u16>().unwrap_or(0),
                    protocol: Some(proto.to_string()),
                });
            } else if part.contains('/') {
                let mut split = part.splitn(2, '/');
                let c_port = split.next().unwrap_or("").parse::<u16>().unwrap_or(0);
                let proto = split.next().unwrap_or("tcp");
                results.push(ContainerPort {
                    host_ip: None,
                    host_port: None,
                    container_port: c_port,
                    protocol: Some(proto.to_string()),
                });
            }
        }
    }

    results
}

pub fn normalize_container_item(raw: &serde_json::Value) -> ContainerItem {
    let cid_full = raw
        .get("Id")
        .or_else(|| raw.get("ID"))
        .or_else(|| raw.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let id = if cid_full.len() > 12 && !cid_full.contains('-') {
        cid_full[..12].to_string()
    } else {
        cid_full.to_string()
    };

    let mut names = Vec::new();
    if let Some(names_val) = raw.get("Names").or_else(|| raw.get("names")) {
        if let Some(s) = names_val.as_str() {
            names = s
                .split(',')
                .map(|n| n.trim().trim_start_matches('/').to_string())
                .filter(|n| !n.is_empty())
                .collect();
        } else if let Some(arr) = names_val.as_array() {
            names = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|n| n.trim_start_matches('/').to_string())
                .filter(|n| !n.is_empty())
                .collect();
        }
    }
    if names.is_empty() {
        names.push(id.clone());
    }

    let image = raw
        .get("Image")
        .or_else(|| raw.get("ImageName"))
        .or_else(|| raw.get("image"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let raw_state = raw
        .get("State")
        .or_else(|| raw.get("state"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let raw_status = raw
        .get("Status")
        .or_else(|| raw.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let state = if raw_status.to_lowercase().contains("up") || raw_state.contains("running") {
        "running".to_string()
    } else if raw_state.contains("paused") || raw_status.to_lowercase().contains("paused") {
        "paused".to_string()
    } else if raw_state.contains("exited")
        || raw_status.to_lowercase().contains("exited")
        || raw_state.contains("stopped")
    {
        "exited".to_string()
    } else if raw_state.contains("created") {
        "created".to_string()
    } else if raw_state.contains("restarting") {
        "restarting".to_string()
    } else if raw_state.contains("dead") {
        "dead".to_string()
    } else if !raw_state.is_empty() {
        raw_state
    } else {
        "unknown".to_string()
    };

    let status = if !raw_status.is_empty() {
        raw_status
    } else {
        let mut c = state.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    };

    let ports = raw
        .get("Ports")
        .or_else(|| raw.get("ports"))
        .map(parse_port_entry)
        .unwrap_or_default();

    let created = raw
        .get("CreatedAt")
        .or_else(|| raw.get("Created"))
        .or_else(|| raw.get("created"))
        .cloned()
        .unwrap_or(serde_json::Value::String("".to_string()));

    let command = raw
        .get("Command")
        .or_else(|| raw.get("command"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    ContainerItem {
        id,
        names,
        image,
        state,
        status,
        created,
        ports,
        command,
    }
}
