// Intentional monomorphization: process<T> is instantiated 4 times with
// different concrete types, each generating a distinct copy of the function body.
fn process<T: std::fmt::Debug + Clone + PartialEq>(items: &[T]) -> Vec<T> {
    items
        .iter()
        .filter(|x| {
            let repr = format!("{:?}", x);
            repr.len() % 2 == 0
        })
        .cloned()
        .collect()
}

fn main() {
    let _ = process(&[1u8, 2, 3, 4, 5, 6, 7, 8]);
    let _ = process(&[1u32, 2, 3, 4]);
    let _ = process(&[1u64, 2, 3, 4, 5, 6]);
    let _ = process(&["hello", "world", "foo", "bar"]);
    let _ = process(&[1.0f64, 2.0, 3.0]);
}
