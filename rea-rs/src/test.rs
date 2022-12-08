use bitvec::prelude::*;

#[test]
fn test() {
    let data: u32 = 0b0110100110;
    println!("{}", data);

    let bits = data.view_bits::<Lsb0>();
    println!("{:?}", bits);

    let back: u32 = bits.load();
    println!("{}", back);
    // let back_to_u32: u32 = u32::from_be_bytes(u32_as_bytes);
    // println!("{}", back_to_u32);
}
