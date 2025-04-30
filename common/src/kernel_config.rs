#[derive(Debug)]
pub struct KernelConfig<'a> {
    pub init_cwd_path: &'a str,
    pub init_app_exec_args: Option<&'a str>,
    pub mouse_pointer_bmp_path: &'a str,
}
