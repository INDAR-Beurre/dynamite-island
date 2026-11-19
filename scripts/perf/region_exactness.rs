// Bit-exactness gate for the fixer's new region upscale: for ANY rect, the
// pixels inside it must equal what the full-stage upscale writes there.
// A mismatch means visible seams at the damage-rect boundary.
include!("region_fns_extracted.rs");
fn main(){
    let sizes=[(1u32,1u32),(1,7),(7,1),(2,2),(3,5),(17,13),(64,48),(110,41)];
    let mut bad=0usize; let mut cases=0usize;
    for &(w,h) in &sizes {
        let n=(w*h*4) as usize;
        let mut src=vec![0u8;n];
        for i in 0..n { src[i]=((i*37+11)%256) as u8; }
        let mut full=vec![0u8;(w*2*h*2*4) as usize];
        upscale_canvas_2x_into(&src,w,h,&mut full);
        // every rect: full, corners, edges, interior, off-by-one past bounds
        let mut rects=vec![(0,0,w as i32,h as i32)];
        let xs:Vec<i32> = if w<=8 {(0..=w as i32).collect()} else {vec![0,1,2,(w/2)as i32,w as i32-1,w as i32]};
        let ys:Vec<i32> = if h<=8 {(0..=h as i32).collect()} else {vec![0,1,2,(h/2)as i32,h as i32-1,h as i32]};
        for &x0 in &xs { for &x1 in &xs { if x1>x0 {
            for &y0 in &ys { for &y1 in &ys { if y1>y0 {
                rects.push((x0,y0,x1,y1));
            }}
        }}}}
        rects.push((-5,-5,w as i32+5,h as i32+5));
        rects.push((w as i32,h as i32,w as i32+3,h as i32+3));
        for r in rects {
            let mut part=vec![0u8;full.len()];
            upscale_canvas_2x_region(&src,w,h,&mut part,r);
            let c=clip_rect(r,w,h);
            if c.2<=c.0||c.3<=c.1 { continue; }
            cases+=1;
            for dy in (c.1*2)..(c.3*2) {
                for dx in (c.0*2)..(c.2*2) {
                    let i=(((dy as u32)*2*w+(dx as u32))*4) as usize;
                    for k in 0..4 {
                        if part[i+k]!=full[i+k] {
                            if bad<6 { println!("MISMATCH {}x{} rect={:?} at dx={} dy={} c={} full={} part={}",w,h,r,dx,dy,k,full[i+k],part[i+k]); }
                            bad+=1;
                        }
                    }
                }
            }
        }
    }
    println!("cases={} mismatches={}",cases,bad);
    assert_eq!(bad,0,"region upscale is NOT bit-exact vs full-stage");
    println!("PASS: bit-exact at every rect tested");
}
