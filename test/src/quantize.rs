use float_eq::assert_float_eq;
use log::info;
use rea_rs::{quantize, Time};
use rea_rs_test::TestStep;

pub fn quantize() -> TestStep {
    TestStep::new("Quantize", |_reaper| {
        info!("test generic u32");
        assert_eq!(quantize::generic_u32(15, 3, 20)?, 21);
        assert_eq!(quantize::generic_u32(15, 3, 19)?, 18);
        assert_eq!(quantize::generic_u32(15, 3, 14)?, 15);
        assert!(quantize::generic_u32(15, 3, 12).is_err());

        info!("test generic f64");
        assert_float_eq!(
            quantize::generic_f64(12.2, 0.3, 13.0)?,
            13.1,
            r2nd <= 0.01
        );
        assert_float_eq!(
            quantize::generic_f64(12.2, 0.3, 12.9)?,
            12.8,
            r2nd <= 0.01
        );
        assert_float_eq!(
            quantize::generic_f64(12.2, 0.3, 12.1)?,
            12.2,
            r2nd <= 0.01
        );
        assert!(quantize::generic_f64(12.2, 0.3, 12.049).is_err());

        info!("test time");
        assert_eq!(
            quantize::by_time(
                Time::from_seconds_f64(12.2)?,
                Time::from_seconds_f64(0.3)?,
                Time::from_seconds_f64(13.0)?,
                None
            )?,
            Time::from_seconds_f64(13.1)?
        );
        assert_eq!(
            quantize::by_time(
                Time::from_seconds_f64(12.2)?,
                Time::from_seconds_f64(0.3)?,
                Time::from_seconds_f64(12.9)?,
                None
            )?,
            Time::from_seconds_f64(12.8)?
        );
        assert_eq!(
            quantize::by_time(
                Time::from_seconds_f64(12.2)?,
                Time::from_seconds_f64(0.3)?,
                Time::from_seconds_f64(12.1)?,
                None
            )?,
            Time::from_seconds_f64(12.2)?
        );
        assert!(quantize::by_time(
            Time::from_seconds_f64(12.2)?,
            Time::from_seconds_f64(0.3)?,
            Time::from_seconds_f64(12.049)?,
            None
        )
        .is_err());

        Ok(())
    })
}
