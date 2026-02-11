use ecore_rs::repr::Pack;

pub struct PackageGenerator<'a> {
    package: &'a Pack,
}

impl<'a> PackageGenerator<'a> {
    pub fn new(package: &'a Pack) -> Self {
        Self { package }
    }

    pub fn generate(&self) -> String {
        self.package.name().to_string()
    }
}
