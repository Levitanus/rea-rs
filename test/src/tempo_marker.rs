use float_eq::assert_float_eq;
use log::debug;
use rea_rs::{
    Measure, Reaper, TempoTimeSigMarker, TempoTimeSigMarkerBasis,
    TempoTimeSigMarkerFlags, Time, TimeSignature,
};
use rea_rs_test::TestStep;

pub fn tempo_marker() -> TestStep {
    TestStep::new("Tempo markers", |_reaper| {
        let reaper = Reaper::get();
        let project = reaper.add_project_tab(true);

        let marker_at_measure_two = TempoTimeSigMarker::add(
            &project,
            Time::from_quarters(4.0, &project)?,
            Some(60.0),
            None,
            false,
            TempoTimeSigMarkerFlags::empty(),
        )?;
        let marker_at_partial_measure = TempoTimeSigMarker::add(
            &project,
            Time::from_quarters_abs(5.5, &project)?,
            None,
            Some(TimeSignature::new(5, 8)),
            false,
            TempoTimeSigMarkerFlags::ALLOW_PREVIOUS_PARTIAL_MEASURE,
        )?;
        let marker_at_measure_four = TempoTimeSigMarker::add(
            &project,
            Time::from_quarters_abs(8.0, &project)?,
            Some(60.0),
            Some(TimeSignature::new(3, 4)),
            false,
            TempoTimeSigMarkerFlags::empty(),
        )?;
        let marker =
            TempoTimeSigMarker::get(&project, marker_at_measure_four)?
                .expect("third tempo marker should exist");
        marker.set_bpm_basis(&project, TempoTimeSigMarkerBasis::Half)?;

        assert_eq!(project.n_tempo_markers()?, 4);

        let marker_zero = TempoTimeSigMarker::get(&project, 0)?
            .expect("default tempo marker should exist");
        assert_eq!(marker_zero.bpm, Some(120.0));
        assert_eq!(
            project.time_signature_at_position(marker_zero.time)?.0,
            TimeSignature::new(4, 4)
        );

        let marker_one =
            TempoTimeSigMarker::get(&project, marker_at_measure_two)?
                .expect("tempo marker at measure two should exist");
        assert_eq!(marker_one.bpm, Some(60.0));
        assert_eq!(
            project.time_signature_at_position(marker_one.time)?.0,
            TimeSignature::new(4, 4)
        );

        let marker_two =
            TempoTimeSigMarker::get(&project, marker_at_partial_measure)?
                .expect("tempo marker at the partial measure should exist");
        assert_eq!(marker_two.bpm, None);
        assert_eq!(marker_two.time_signature, Some(TimeSignature::new(5, 8)));
        assert!(marker_two.flags.contains(
            TempoTimeSigMarkerFlags::ALLOW_PREVIOUS_PARTIAL_MEASURE
        ));

        let marker_three =
            TempoTimeSigMarker::get(&project, marker_at_measure_four)?
                .expect("tempo marker at measure four should exist");
        assert!(matches!(marker_three.bpm, Some(60.0)));
        assert_eq!(
            marker_three.time_signature,
            Some(TimeSignature::new(3, 4))
        );
        assert_eq!(
            marker_three.get_bpm_basis(&project)?,
            TempoTimeSigMarkerBasis::Half
        );

        let expected_measures = [
            (0.0, 4.0, TimeSignature::new(4, 4), 120.0),
            (4.0, 5.5, TimeSignature::new(4, 4), 60.0),
            (5.5, 8.0, TimeSignature::new(5, 8), 60.0),
        ];
        for (
            index,
            (expected_start, expected_end, expected_signature, expected_tempo),
        ) in expected_measures.into_iter().enumerate()
        {
            let measure = Measure::from_index(index as u32, &project)?;
            debug!("checking measure: {:#?}", measure);
            assert_float_eq!(
                measure.start.as_quarters_abs(&project)?,
                expected_start,
                abs <= 0.0001
            );
            assert_float_eq!(
                measure.end.as_quarters_abs(&project)?,
                expected_end,
                abs <= 0.0001
            );
            assert_eq!(measure.time_signature, expected_signature);
            assert_float_eq!(measure.tempo, expected_tempo, abs <= 0.0001);
        }

        let measure_three = Measure::from_index(3, &project)?;
        assert_float_eq!(
            measure_three.start.as_quarters_abs(&project)?,
            8.0,
            abs <= 0.0001
        );
        assert_float_eq!(
            measure_three.end.as_quarters_abs(&project)?,
            11.0,
            abs <= 0.0001
        );
        assert_eq!(measure_three.time_signature, TimeSignature::new(3, 4));
        assert!(matches!(measure_three.tempo, 60.0 | 120.0));

        project.close()?;
        Ok(())
    })
}
