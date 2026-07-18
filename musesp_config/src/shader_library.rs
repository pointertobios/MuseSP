use std::collections::HashMap;
use std::sync::Arc;
use wgpu::ShaderModule;

/// 预编译的所有 shader 模块，按 label 索引。
pub struct ShaderLibrary {
    modules: HashMap<String, Arc<ShaderModule>>,
}

impl ShaderLibrary {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut modules = HashMap::new();

        let compile = |label: &str, source: &str| -> Arc<ShaderModule> {
            Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(source)),
            }))
        };

        let insert = |modules: &mut HashMap<_, _>, label: &str, source: &str| {
            modules.insert(label.to_string(), compile(label, source));
        };

        insert(
            &mut modules,
            "surface_eval",
            include_str!("../../musesp/src/gameplay/shader_pass1_eval.wgsl"),
        );
        insert(
            &mut modules,
            "surface_final",
            include_str!("../../musesp/src/gameplay/shader_pass1_final.wgsl"),
        );
        insert(
            &mut modules,
            "surface_pass2",
            include_str!("../../musesp/src/gameplay/shader_pass2.wgsl"),
        );
        insert(
            &mut modules,
            "line_eval",
            include_str!("../../musesp/src/gameplay/line_subdivide_eval.wgsl"),
        );
        insert(
            &mut modules,
            "line_final",
            include_str!("../../musesp/src/gameplay/line_subdivide_final.wgsl"),
        );
        insert(
            &mut modules,
            "line_render",
            include_str!("../../musesp/src/gameplay/line.wgsl"),
        );

        // UI 层 shader
        insert(
            &mut modules,
            "rect",
            include_str!("../../musesp_ui/src/shaders/rect.wgsl"),
        );
        insert(
            &mut modules,
            "texture",
            include_str!("../../musesp_ui/src/shaders/texture.wgsl"),
        );

        ShaderLibrary { modules }
    }

    pub fn get(&self, label: &str) -> &Arc<ShaderModule> {
        self.modules
            .get(label)
            .unwrap_or_else(|| panic!("Shader '{label}' not found in library"))
    }
}
