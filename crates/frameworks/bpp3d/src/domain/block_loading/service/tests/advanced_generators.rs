    #[test]
    fn advanced_block_generators_report_diagnostics() {
        assert!(ComplexBlockGenerator::default()
            .diagnostics()[0]
            .contains("complex block generation"));
        assert!(DepthFirstSearchAlgorithm::default()
            .diagnostics()[0]
            .contains("depth-first search"));
        assert!(MultiLayerHeuristicSearchAlgorithm::default()
            .diagnostics()[0]
            .contains("multi-layer heuristic"));
    }

    #[test]
    fn advanced_block_generators_produce_bounded_search_results() {
        let items = vec![
            make_cuboid_item("i1", 2.0, 2.0, 2.0, 1.0),
            make_cuboid_item("i2", 2.0, 2.0, 2.0, 1.0),
        ];
        let amounts = vec![2u64, 2u64];
        let container = MetricSize3 {
            width: meters(5.0),
            height: meters(5.0),
            depth: meters(5.0),
        };
        let simple_blocks = SimpleBlockGenerator::default_generator()
            .generate(&items, &amounts, &container);
        let complex_blocks = ComplexBlockGenerator::default()
            .generate(&amounts, &container, &simple_blocks, None);
        let dfs_blocks = simple_blocks
            .iter()
            .chain(complex_blocks.iter())
            .cloned()
            .collect::<Vec<_>>();
        let placements = DepthFirstSearchAlgorithm::default().pack(&dfs_blocks, &container);
        let layers = MultiLayerHeuristicSearchAlgorithm::default()
            .pack_layers(&dfs_blocks, &container);

        assert!(!complex_blocks.is_empty());
        assert!(complex_blocks.iter().any(|block| matches!(block, Block::Complex(_))));
        assert!(!placements.is_empty());
        assert!(!layers.is_empty());
    }

    #[test]
    fn complex_block_generator_builds_multi_round_composites() {
        let items = vec![make_cuboid_item("i1", 1.0, 1.0, 1.0, 1.0)];
        let amounts = vec![4u64];
        let container = MetricSize3 {
            width: meters(4.0),
            height: meters(1.0),
            depth: meters(1.0),
        };
        let simple_blocks = SimpleBlockGenerator::default_generator()
            .generate(&items, &amounts, &container);
        let complex_blocks = ComplexBlockGenerator::default()
            .generate(&amounts, &container, &simple_blocks, None);

        assert!(complex_blocks.iter().any(|block| match block {
            Block::Complex(complex) => complex.sub_blocks.len() >= 3,
            Block::Simple(_) => false,
        }));
    }

    #[test]
    fn dfs_and_mlhs_rank_candidate_volume() {
        let items = vec![
            make_cuboid_item("big", 3.0, 2.0, 1.0, 1.0),
            make_cuboid_item("small", 2.0, 2.0, 1.0, 1.0),
        ];
        let amounts = vec![1u64, 2u64];
        let container = MetricSize3 {
            width: meters(4.0),
            height: meters(2.0),
            depth: meters(1.0),
        };
        let blocks = SimpleBlockGenerator::default_generator()
            .generate(&items, &amounts, &container);
        let dfs = DepthFirstSearchAlgorithm::default();
        let candidates = dfs.pack_candidates(&blocks, &container);
        let best_volume = candidates
            .first()
            .map(|placements| placements_volume(&blocks, placements))
            .unwrap_or(0.0);
        let mlhs_candidates = MultiLayerHeuristicSearchAlgorithm::default()
            .pack_layer_candidates(&blocks, &container);

        assert!(best_volume >= 8.0);
        assert!(!mlhs_candidates.is_empty());
    }

    #[test]
    fn dfs_and_mlhs_accept_axis_aware_cylinder_blocks() {
        let items = vec![
            make_cylinder_item("cx", 2.0, 8.0, Axis3::X, 1.0),
            make_cylinder_item("cz", 2.0, 9.0, Axis3::Z, 1.0),
        ];
        let amounts = vec![1u64, 1u64];
        let container = MetricSize3 {
            width: meters(12.0),
            height: meters(8.0),
            depth: meters(12.0),
        };
        let blocks = SimpleBlockGenerator::default_generator()
            .generate(&items, &amounts, &container);
        let dfs = DepthFirstSearchAlgorithm::default();
        let dfs_candidates = dfs.pack_candidates(&blocks, &container);
        let mlhs_candidates = MultiLayerHeuristicSearchAlgorithm::default()
            .pack_layer_candidates(&blocks, &container);

        let dfs_volume = dfs_candidates
            .first()
            .map(|placements| placements_volume(&blocks, placements))
            .unwrap_or(0.0);
        assert!(dfs_volume > 0.0);
        assert!(dfs_candidates.iter().flatten().any(|placement| match &blocks[placement.block_index] {
            Block::Simple(simple) => simple.item_view.packing_shape.axis == Some(Axis3::X),
            Block::Complex(_) => false,
        }));
        assert!(dfs_candidates.iter().flatten().any(|placement| match &blocks[placement.block_index] {
            Block::Simple(simple) => simple.item_view.packing_shape.axis == Some(Axis3::Z),
            Block::Complex(_) => false,
        }));
        assert!(!mlhs_candidates.is_empty());
        assert!(mlhs_candidates
            .iter()
            .flatten()
            .flatten()
            .any(|placement| match &blocks[placement.block_index] {
                Block::Simple(simple) => {
                    matches!(simple.item_view.packing_shape.axis, Some(Axis3::X | Axis3::Z))
                }
                Block::Complex(_) => false,
            }));
    }

    #[test]
    fn complex_block_frontier_matches_reference_for_limits_and_full_payloads() {
        let items = vec![make_cuboid_item("a", 1.0, 1.0, 1.0, 1.0),
            make_cuboid_item("b", 2.0, 1.0, 1.0, 2.0),
            make_cylinder_item("c", 0.5, 1.0, Axis3::Z, 1.0)];
        let container = MetricSize3 { width: meters(8.0), height: meters(4.0), depth: meters(3.0) };
        let mut blocks = SimpleBlockGenerator::default_generator().generate(&items, &[6, 4, 2], &container);
        blocks.extend(ComplexBlockGenerator::default().generate(&[6, 4, 2], &container, &blocks, None).into_iter().take(2));
        blocks.push(blocks[0].clone());
        for mask in 0..8 {
            for max_candidates in [0, 1, 2, 7, 100] {
                for max_depth in [0, 1, 2, 4] {
                    for rest_weight in [None, Some(0.0), Some(3.0), Some(30.0)] {
                        let generator = ComplexBlockGenerator::new(ComplexBlockGeneratorConfig {
                            with_x: mask & 1 != 0, with_y: mask & 2 != 0, with_z: mask & 4 != 0,
                            max_candidates, max_depth,
                        });
                        for amounts in [&[6, 4, 2][..], &[0, 0, 0][..], &[1][..]] {
                            let expected = generator.generate_reference(amounts, &container, &blocks, rest_weight);
                            let actual = generator.generate(amounts, &container, &blocks, rest_weight);
                            assert_eq!(format!("{actual:?}"), format!("{expected:?}"),
                                "mask={mask} max={max_candidates} depth={max_depth} weight={rest_weight:?}");
                        }
                    }
                }
            }
        }
        assert!(ComplexBlockGenerator::default().generate(&[], &container, &[], None).is_empty());
    }

    #[test]
    #[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
    fn performance_b10_complex_generation() {
        let items = vec![make_cuboid_item("a", 1.0, 1.0, 1.0, 1.0),
            make_cuboid_item("b", 2.0, 1.0, 1.0, 1.0), make_cuboid_item("c", 1.0, 2.0, 1.0, 1.0)];
        let amounts = [100, 100, 100];
        let container = MetricSize3 { width: meters(20.0), height: meters(20.0), depth: meters(20.0) };
        let blocks = SimpleBlockGenerator::default_generator().generate(&items, &amounts, &container);
        for limit in [64, 256, 1024] {
            let generator = ComplexBlockGenerator::new(ComplexBlockGeneratorConfig {
                with_z: true, max_candidates: limit, max_depth: 4, ..Default::default()
            });
            for sample in 0..5 {
                for reference in if sample % 2 == 0 { [true, false] } else { [false, true] } {
                    let started = std::time::Instant::now();
                    let result = if reference { generator.generate_reference(&amounts, &container, std::hint::black_box(&blocks), None) }
                        else { generator.generate(&amounts, &container, std::hint::black_box(&blocks), None) };
                    let elapsed = started.elapsed();
                    eprintln!("complex_generation limit={limit} reference={reference} sample={sample} elapsed={elapsed:?} results={}", result.len());
                    std::hint::black_box(result);
                }
            }
        }
    }
