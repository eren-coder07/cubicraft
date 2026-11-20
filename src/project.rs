// CubiCraft — project files.
// Open, documented, JSON-based formats that mirror what .cmo3 / .can3 do for
// the proprietary Cubism Editor, without using their bytes:
//   * .ccm3  — a CubiCraft model: parameters, deformer tree, physics, clips
//   * .cca3  — recorded animation clips (embedded in .ccm3; standalone later)

use crate::deform::Node;
use crate::motion::Clip;
use crate::params::{ParamDef, Params};
use crate::physics::PhysicsSetting;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub format: String, // "ccm3"
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub custom_params: Vec<ParamDef>,
    pub root: Node,
    pub physics: Vec<PhysicsSetting>,
    pub clips: Vec<Clip>,
}

impl Project {
    /// Build the project that describes the built-in Momi rig.
    pub fn momi(params: &Params, root: Node, physics: Vec<PhysicsSetting>, clips: Vec<Clip>) -> Project {
        let custom_params = params
            .defs
            .iter()
            .enumerate()
            .filter(|(i, d)| *i >= BASE_PARAM_COUNT)
            .map(|(_, d)| d.clone())
            .collect();
        Project {
            format: "ccm3".into(),
            version: 1,
            name: "Momi".into(),
            custom_params,
            root,
            physics,
            clips,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialize project")
    }

    pub fn from_json(json: &str) -> Result<Project, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid .ccm3: {e}"))
    }

    /// Apply custom params onto the runtime parameter store.
    pub fn install_params(&self, params: &mut Params) {
        params.reset_to_base();
        for d in &self.custom_params {
            params.add_param(d.id.clone(), d.name.clone(), d.group.clone(), d.min, d.def, d.max);
        }
    }
}

/// Number of built-in parameters (custom ones are appended after these).
pub const BASE_PARAM_COUNT: usize = 31;

/// Save to disk (native builds). Returns an error message on failure.
pub fn save_file(project: &Project, path: &str) -> Result<(), String> {
    let json = project.to_json();
    std::fs::write(path, json).map_err(|e| format!("save failed: {e}"))
}

/// Load from disk (native builds).
pub fn load_file(path: &str) -> Result<Project, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("open failed: {e}"))?;
    Project::from_json(&json)
}
