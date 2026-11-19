include!("morph_rect_fns.rs");
use std::time::Instant;
fn main(){
    let (w,h)=(1176u32,636u32);
    let src=vec![7u8;(w*h*4) as usize];
    let mut dst=vec![0u8;(w*h*4*4) as usize];
    // realistic morph rects: collapsed pill -> controlcenter panel
    // stage coords; pill ~ (554,13,67,25); cc panel ~ (388,13,400,150) padded 39
    let cases:[(&str,(i32,i32,i32,i32));4]=[
        ("full stage",(0,0,w as i32,h as i32)),
        ("collapsed pill only",(552,11,623,40)),
        ("pill->cc union",(349,0,827,191)),
        ("cc->themes union",(300,0,876,340)),
    ];
    for (name,r) in cases {
        let r=clip_rect(r,w,h);
        let mut best=std::time::Duration::MAX;
        for _ in 0..5 {
            let t=Instant::now();
            upscale_canvas_2x_region(&src,w,h,&mut dst,r);
            best=best.min(t.elapsed());
        }
        let px=((r.2-r.0)*(r.3-r.1)) as f64;
        println!("{:22} rect {:?}  {:8.3} ms   ({:.0} logical px)",name,r,best.as_secs_f64()*1000.0,px);
    }
    let mut best=std::time::Duration::MAX;
    for _ in 0..5 { let t=Instant::now(); upscale_canvas_2x_into(&src,w,h,&mut dst); best=best.min(t.elapsed()); }
    println!("{:22} {:8.3} ms","FULL (old path)",best.as_secs_f64()*1000.0);
}
