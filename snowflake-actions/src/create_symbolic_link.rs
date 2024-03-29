use {
    anyhow::Context,
    os_ext::{cstring, symlinkat},
    snowflake_core::action::{
        Action, InputPath, Outputs,
        Perform, Result, Success,
    },
    snowflake_util::hash::{Blake3, Hash},
    std::ffi::CString,
};

/// Action that creates a symbolic link.
pub struct CreateSymbolicLink
{
    /// The target of the symbolic link.
    pub target: CString,
}

impl Action for CreateSymbolicLink
{
    fn inputs(&self) -> usize
    {
        0
    }

    fn outputs(&self) -> Outputs<usize>
    {
        Outputs::Outputs(1)
    }

    fn perform(&self, perform: &Perform, input_paths: &[InputPath]) -> Result
    {
        debug_assert_eq!(input_paths.len(), 0);
        let output_path = cstring!(b"output");
        symlinkat(&self.target, Some(perform.scratch), &output_path)
            .context("Create symbolic link")?;
        Ok(Success{output_paths: vec![output_path], warnings: false})
    }

    fn hash(&self, input_hashes: &[Hash]) -> Hash
    {
        // NOTE: See the manual chapter on avoiding hash collisions.

        let Self{target} = self;

        debug_assert_eq!(input_hashes.len(), 0);

        let mut h = Blake3::new();
        h.put_str("CreateSymbolicLink");
        h.put_cstr(target);
        h.finalize()
    }
}
