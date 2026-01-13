mod bindings {
    wit_bindgen::generate!({
        inline: "
        package legacy:exec;
        interface exec {
          exec: func(action: string, args: string) -> string;
        }
        world exec-world {
          export exec;
        }",
        world: "exec-world",
        generate_all,
    });
}

use bindings::exports::legacy::exec::exec::Guest;

#[allow(dead_code)]
struct Legacy;

impl Guest for Legacy {
    fn exec(_action: String, args: String) -> String {
        args
    }
}

#[cfg(target_arch = "wasm32")]
bindings::export!(Legacy with_types_in bindings);
