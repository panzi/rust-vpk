#[derive(Debug)]
pub enum Filter<'a> {
    None,
    Paths(Vec<&'a str>)
}
