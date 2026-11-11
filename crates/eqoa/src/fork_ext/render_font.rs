// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Enhanced-only logical font sizing, compiled as a child of render::ui so
// shared rendering keeps its original font upload and a single adapter call.
impl super::Font {
    /// Keep atlas texels and UVs while changing only logical layout metrics.
    pub(super) fn at_height(mut self, height: u32) -> Self {
        if height != self.height {
            let height = height.max(1);
            let source_height = self.height.max(1);
            for (width, _) in self.glyphs.values_mut() {
                *width = (u64::from(*width) * u64::from(height))
                    .div_ceil(u64::from(source_height))
                    .min(u64::from(u32::MAX)) as u32;
            }
            self.height = height;
        }
        self
    }

}

pub(super) fn apply_last_height(ui: &mut super::Ui, height: u32) {
    let font = ui.fonts.pop().expect("the just-uploaded font").at_height(height);
    ui.fonts.push(font);
}
