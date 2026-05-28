#[cfg(test)]
mod tests {
    use minijinja::Environment;

    use crate::modules::plugin::Plugin;
    use crate::plugins::hello_world::HelloWorldPlugin;

    #[test]
    fn hello_world_has_correct_name_and_version() {
        let plugin = HelloWorldPlugin::new();
        assert_eq!(plugin.name(), "hello-world");
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn hello_world_extend_template_env_registers_function() {
        let plugin = HelloWorldPlugin::new();
        let mut env = Environment::new();

        plugin.extend_template_env(&mut env).unwrap();

        let result = env
            .render_str("{{ hello_world() }}", minijinja::context!())
            .unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn hello_world_template_function_with_name_arg() {
        let plugin = HelloWorldPlugin::new();
        let mut env = Environment::new();

        plugin.extend_template_env(&mut env).unwrap();

        let result = env
            .render_str("{{ hello_world('InkForge') }}", minijinja::context!())
            .unwrap();
        assert_eq!(result, "Hello, InkForge!");
    }

    #[test]
    fn hello_world_api_routes_router_created_without_panic() {
        let plugin = HelloWorldPlugin::new();
        let router = plugin.api_routes();
        let debug = format!("{:?}", router);
        assert!(
            debug.contains("plugins"),
            "plugin router should contain /api/v1/plugins path, got: {}",
            debug
        );
    }
}
