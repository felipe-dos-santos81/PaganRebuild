//! Letting time pass: t<n> runs n ticks, w<dir>[r]:<n> walks (r: runs) n ticks in a direction.

use super::*;

pub(super) fn run(game: &mut Game, surf: &mut Surface, opts: &mut ViewOptions, _st: &mut State, name: &str, arg: &str, cmd: &str) -> Result<Flow, String> {
    let num = |s: &str| number(s, cmd);
    if let Some(n) = name.strip_prefix('t') {
        // PAGAN_TICKTIME=1: report every tick that takes longer than 5 ms
        let timed = crate::debug::env().tick_time;
        for _ in 0..num(n)? {
            let start = std::time::Instant::now();
            game.step(None);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            if timed && ms > 5.0 {
                println!("ticktime: tick {} took {ms:.1} ms", game.tick);
            }
        }
    } else if let Some(d) = name.strip_prefix('w') {
        let run = d.ends_with('r');
        let dir = num(d.trim_end_matches('r'))? as u8 & 7;
        // PAGAN_TICKTIME=1: each tick and a picture after it are timed;
        // the slow ones are reported, and the average and worst at the end
        let timed = crate::debug::env().tick_time;
        let (mut steps, mut draws) = (Vec::new(), Vec::new());
        for _ in 0..num(arg)? {
            let start = std::time::Instant::now();
            game.step(Some(MoveInput { dir, run }));
            if !timed {
                continue;
            }
            let step_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = std::time::Instant::now();
            game.draw(surf, opts, false);
            let draw_ms = start.elapsed().as_secs_f64() * 1000.0;
            if step_ms > 5.0 || draw_ms > 8.0 {
                let av = game.avatar();
                println!("ticktime: tick {} step {step_ms:.1} ms, picture {draw_ms:.1} ms at {},{},{}", game.tick, av.x, av.y, av.z);
            }
            steps.push(step_ms);
            draws.push(draw_ms);
        }
        if timed && !steps.is_empty() {
            let avg = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
            let worst = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
            println!("ticktime: {} ticks, step {:.2} ms (worst {:.1}), picture {:.2} ms (worst {:.1})", steps.len(), avg(&steps), worst(&steps), avg(&draws), worst(&draws));
        }
    } else {
        return Ok(Flow::Unknown);
    }
    Ok(Flow::Done)
}
