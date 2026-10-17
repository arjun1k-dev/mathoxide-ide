//! mathoxide-mcp — Model Context Protocol (MCP) server for MathOxide.
//! Exposes modular asset inspection, code generation, validation, and rendering to AI agents.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
            if let Some(resp) = handle_request(&req) {
                let resp_str = serde_json::to_string(&resp)?;
                writeln!(writer, "{}", resp_str)?;
                writer.flush()?;
            }
        }
        line.clear();
    }

    Ok(())
}

fn handle_request(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    match req.method.as_str() {
        "initialize" => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mathoxide-mcp",
                    "version": "0.1.0"
                }
            })),
            error: None,
        }),
        "notifications/initialized" => None,
        "ping" => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(json!({})),
            error: None,
        }),
        "tools/list" => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(json!({
                "tools": [
                    {
                        "name": "list_assets",
                        "description": "List all modular .moa animation assets and master compositions in the project with tags, line counts, durations, and a per-play timeline (start/end in seconds) so overlapping clips are known, not guessed.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "dir": {
                                    "type": "string",
                                    "description": "Optional project root directory (defaults to current dir)"
                                }
                            }
                        }
                    },
                    {
                        "name": "read_asset",
                        "description": "Read the source code and structural details of a specific .moa asset or composition file, including its per-play timeline with exact start/end times.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Path to the .moa file"
                                }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "write_asset",
                        "description": "Create or overwrite an isolated component asset (typically in assets/) or a master composition (in examples/ or root).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Path to the .moa file to write"
                                },
                                "code": {
                                    "type": "string",
                                    "description": "MathOxide DSL source code for the asset"
                                }
                            },
                            "required": ["path", "code"]
                        }
                    },
                    {
                        "name": "validate_code",
                        "description": "Validate MathOxide DSL code without running a GUI. Returns syntax diagnostics, duration, initial objects, and timeline animations.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "code": {
                                    "type": "string",
                                    "description": "MathOxide DSL code to validate"
                                }
                            },
                            "required": ["code"]
                        }
                    },
                    {
                        "name": "render_frame",
                        "description": "Render a specific frame (at time t in seconds) from a .moa file or code string directly to a PNG image.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Path to the .moa file to render"
                                },
                                "out": {
                                    "type": "string",
                                    "description": "Output PNG file path"
                                },
                                "time": {
                                    "type": "number",
                                    "description": "Timestamp in seconds (default: 0.0)"
                                },
                                "width": {
                                    "type": "integer",
                                    "description": "Resolution width (default: 1280)"
                                },
                                "height": {
                                    "type": "integer",
                                    "description": "Resolution height (default: 720)"
                                }
                            },
                            "required": ["path", "out"]
                        }
                    },
                    {
                        "name": "assemble_scene",
                        "description": "Generate a master composition file that imports and sequences several modular component assets into a unified timeline.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "out_path": {
                                    "type": "string",
                                    "description": "Destination file for the master composition (e.g. 'examples/master_video.moa')"
                                },
                                "title": {
                                    "type": "string",
                                    "description": "Title text for the scene"
                                },
                                "imports": {
                                    "type": "array",
                                    "description": "List of assets to import with aliases and coordinates",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "path": { "type": "string" },
                                            "alias": { "type": "string" },
                                            "at": { "type": "array", "items": { "type": "number" } }
                                        },
                                        "required": ["path", "alias"]
                                    }
                                }
                            },
                            "required": ["out_path", "imports"]
                        }
                    }
                ]
            })),
            error: None,
        }),
        "tools/call" => {
            let empty = json!({});
            let params = req.params.as_ref().unwrap_or(&empty);
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").unwrap_or(params);

            let result = execute_tool(tool_name, args);
            Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: req.id.clone(),
                result: match result {
                    Ok(val) => Some(json!({
                        "content": [
                            {
                                "type": "text",
                                "text": val.to_string()
                            }
                        ]
                    })),
                    Err(e) => Some(json!({
                        "isError": true,
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {e}")
                            }
                        ]
                    })),
                },
                error: None,
            })
        }
        _ => Some(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: None,
            error: Some(json!({
                "code": -32601,
                "message": format!("Method not found: {}", req.method)
            })),
        }),
    }
}

/// Short human/agent-readable label for a play animation.
fn anim_label(a: &mo_core::anim::Animation) -> String {
    use mo_core::anim::Animation;
    match a {
        Animation::ShowCreation { id } => format!("showCreation({id})"),
        Animation::FadeIn { id, .. } => format!("fadeIn({id})"),
        Animation::FadeOut { id, .. } => format!("fadeOut({id})"),
        Animation::Grow { id, .. } => format!("growFromCenter({id})"),
        Animation::Rotate { id, .. } => format!("rotate({id})"),
        Animation::SpinInFromNothing { id } => format!("spinInFromNothing({id})"),
        Animation::ShrinkToCenter { id } => format!("shrinkToCenter({id})"),
        Animation::Indicate { id, .. } => format!("indicate({id})"),
        Animation::Wiggle { id, .. } => format!("wiggle({id})"),
        Animation::Transform { id, .. } => format!("transform({id})"),
        Animation::CameraOrbit { .. } => "camera.orbit".into(),
        Animation::CameraZoom { .. } => "camera.zoom".into(),
        Animation::CameraMoveTo { .. } => "camera.move_to".into(),
    }
}

/// Per-play start/end/overlap map of a timeline, in flat play order. Lets an
/// agent know exactly which asset clips overlap instead of guessing.
fn timeline_json(tl: &mo_core::scene::Timeline) -> Value {
    let mut t = 0.0f32;
    let entries: Vec<Value> = tl
        .stmts
        .iter()
        .filter_map(|s| match s {
            mo_core::scene::Stmt::Play(a) => {
                let e = json!({
                    "start": (t * 100.0).round() / 100.0,
                    "end": ((t + a.run_time) * 100.0).round() / 100.0,
                    "run_time": a.run_time,
                    "anim": anim_label(&a.anim),
                });
                t += a.run_time;
                Some(e)
            }
            mo_core::scene::Stmt::Wait(d) => {
                t += d;
                None
            }
            mo_core::scene::Stmt::Decl { .. } => None,
        })
        .collect();
    json!(entries)
}

fn execute_tool(name: &str, args: &Value) -> anyhow::Result<Value> {    match name {
        "list_assets" => {
            let base_dir = args.get("dir").and_then(|v| v.as_str()).unwrap_or(".");
            let project = PathBuf::from(base_dir);
            let dirs = [project.join("assets"), project.join("examples"), project.clone()];

            let mut assets = Vec::new();
            let mut seen = std::collections::HashSet::new();

            for dir in &dirs {
                if !dir.exists() {
                    continue;
                }
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().map_or(false, |ext| ext == "moa") {
                            if seen.insert(path.clone()) {
                                if let Ok(code) = std::fs::read_to_string(&path) {
                                    let is_master = code.contains("import ");
                                    let parsed = mo_dsl::parse(&code);
                                    let duration = parsed.as_ref().map(|tl| tl.duration()).unwrap_or(0.0);
                                    let timeline = parsed.map(|tl| timeline_json(&tl)).unwrap_or(json!([]));
                                    let line_count = code.lines().count();
                                    assets.push(json!({
                                        "path": path.to_string_lossy(),
                                        "filename": path.file_name().unwrap_or_default().to_string_lossy(),
                                        "is_master": is_master,
                                        "lines": line_count,
                                        "duration": duration,
                                        "timeline": timeline,
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            Ok(json!({ "assets": assets, "count": assets.len() }))
        }
        "read_asset" => {
            let path_str = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let code = std::fs::read_to_string(path_str)?;
            let parsed = mo_dsl::parse(&code);
            let (is_valid, duration, objects, timeline) = match parsed {
                Ok(tl) => {
                    let timeline = timeline_json(&tl);
                    (true, tl.duration(), tl.initial_objects().keys().cloned().collect::<Vec<_>>(), timeline)
                }
                Err(_) => (false, 0.0, Vec::new(), json!([])),
            };

            Ok(json!({
                "path": path_str,
                "code": code,
                "is_valid": is_valid,
                "duration": duration,
                "objects": objects,
                "timeline": timeline
            }))
        }
        "write_asset" => {
            let path_str = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let code = args.get("code").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing code"))?;
            let path = Path::new(path_str);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(path, code)?;

            let validation = mo_dsl::parse(code);
            let summary = match validation {
                Ok(tl) => format!("Saved successfully. Valid syntax, duration: {:.2}s, {} objects.", tl.duration(), tl.initial_objects().len()),
                Err(diags) => format!("Saved, but contains syntax warnings: {:?}", diags),
            };

            Ok(json!({ "status": "ok", "path": path_str, "summary": summary }))
        }
        "validate_code" => {
            let code = args.get("code").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing code"))?;
            match mo_dsl::parse(code) {
                Ok(tl) => {
                    let objects = tl.initial_objects().keys().cloned().collect::<Vec<_>>();
                    Ok(json!({
                        "valid": true,
                        "duration": tl.duration(),
                        "statements": tl.stmts.len(),
                        "objects": objects
                    }))
                }
                Err(diags) => {
                    let errs: Vec<_> = diags.iter().map(|d| format!("line {}:{}: {}", d.line, d.col, d.message)).collect();
                    Ok(json!({
                        "valid": false,
                        "errors": errs
                    }))
                }
            }
        }
        "render_frame" => {
            let path_str = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let out_str = args.get("out").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing out"))?;
            let t = args.get("time").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
            let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(720) as u32;

            let code = std::fs::read_to_string(path_str)?;
            let tl = mo_dsl::parse(&code).map_err(|d| anyhow::anyhow!("{d:?}"))?;
            let renderer = mo_render::Renderer::offscreen(width, height)?;
            let mut camera = mo_core::camera::Camera::default();
            camera.width = width as f32;
            camera.height = height as f32;

            let state = mo_core::scene::Player::state_at(&tl, t);
            let img = renderer.render_with_camera(&state, &camera);
            img.save(out_str)?;

            Ok(json!({
                "status": "rendered",
                "output": out_str,
                "time": t,
                "resolution": format!("{width}x{height}")
            }))
        }
        "assemble_scene" => {
            let out_path = args.get("out_path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing out_path"))?;
            let title = args.get("title").and_then(|v| v.as_str());
            let imports = args.get("imports").and_then(|v| v.as_array()).ok_or_else(|| anyhow::anyhow!("missing imports"))?;

            let mut out_code = String::new();
            out_code.push_str("// Assembled Scene Composition\n");
            if let Some(t) = title {
                out_code.push_str(&format!("text title text=\"{t}\" at (0, 3.4) size=0.7 color=white;\n"));
                out_code.push_str("play write(title) 1.0s smooth;\n\n");
            }

            for imp in imports {
                let p = imp.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let alias = imp.get("alias").and_then(|v| v.as_str()).unwrap_or("clip");
                let at = imp.get("at").and_then(|v| v.as_array());
                let (x, y) = if let Some(arr) = at {
                    let ax = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let ay = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    (ax, ay)
                } else {
                    (0.0, 0.0)
                };

                out_code.push_str(&format!("import \"{p}\" as {alias} at ({x}, {y});\n"));
            }

            out_code.push_str("\nwait 1.0s;\n");

            let path = Path::new(out_path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(path, &out_code)?;

            Ok(json!({
                "status": "assembled",
                "out_path": out_path,
                "code": out_code
            }))
        }
        other => anyhow::bail!("unknown tool `{other}`"),
    }
}
