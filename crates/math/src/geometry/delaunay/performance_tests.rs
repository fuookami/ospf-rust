use super::*;

fn signature(edges: &[Edge2]) -> Vec<[u64; 4]> {
    edges.iter().map(|edge| {
        let (a, b) = edge.clone().into_points();
        [a.x().to_bits(), a.y().to_bits(), b.x().to_bits(), b.y().to_bits()]
    }).collect()
}

fn verify(result: &DelaunayTriangulation) {
    for (triangle, actual) in result.triangles.iter().zip(result.point_indices()) {
        let expected01 = result.find_point_indices_reference(triangle.p1(), triangle.p2());
        let expected2 = result.find_point_indices_reference(triangle.p3(), triangle.p3()).0;
        assert_eq!(*actual, [expected01.0, expected01.1, expected2]);
    }
    assert_eq!(signature(&result.edges()), signature(&result.edges_reference()));
}

fn fixture(count: usize) -> DelaunayTriangulation {
    let points = (0..count).map(|i| Point2::new(i as f64, (i * i % 97) as f64)).collect::<Vec<_>>();
    let triangles = (2..count).map(|i| Triangle2::new(points[0].clone(), points[i - 1].clone(), points[i].clone())).collect();
    DelaunayTriangulation::new(triangles, points)
}

#[test]
fn indexed_edges_preserve_last_approximate_match_and_order() {
    let epsilon = <f64 as crate::algebra::Epsilon>::epsilon();
    for offset in [0.0, -0.0, epsilon * 0.5, epsilon, epsilon * 1.5, -epsilon * 0.5] {
        let points = vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0), Point2::new(0.0, 1.0),
            Point2::new(offset, 0.0), Point2::new(epsilon * 0.75, 0.0)];
        let triangles = vec![Triangle2::new(points[0].clone(), points[1].clone(), points[2].clone()),
            Triangle2::new(points[3].clone(), points[2].clone(), points[1].clone()),
            Triangle2::new(points[4].clone(), points[1].clone(), points[2].clone())];
        let result = DelaunayTriangulation::new(triangles, points);
        verify(&result);
        assert_eq!(signature(&result.edges()), signature(&result.edges_reference()));
        assert_eq!(signature(&result.clone().edges()), signature(&result.edges_reference()));
    }
    let a = Point2::new(0.0, 1.0);
    let b = Point2::new(f64::from_bits(1.0f64.to_bits().wrapping_mul(31)), 0.0);
    assert_eq!(hash_point(&a), hash_point(&b));
    let points = vec![a.clone(), b.clone(), Point2::new(2.0, 2.0), Point2::new(f64::NAN, 0.0)];
    let triangles = vec![Triangle2::new(a, b, points[2].clone()),
        Triangle2::new(points[3].clone(), points[2].clone(), points[0].clone())];
    let result = DelaunayTriangulation::new(triangles, points);
    verify(&result);
    assert_eq!(signature(&result.edges()), signature(&result.edges_reference()));
    let mut state = 981357u64;
    for count in [3, 8, 20, 64] {
        let points = (0..count).map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (state >> 32) as f64 / u32::MAX as f64;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            Point2::new(x, (state >> 32) as f64 / u32::MAX as f64)
        }).collect::<Vec<_>>();
        let result = delaunay_triangulate(&points).unwrap();
        verify(&result);
        assert_eq!(result.triangles(), triangulate_reference(&points).unwrap().triangles());
        assert_eq!(signature(&result.edges()), signature(&result.edges_reference()));
    }
}

#[test]
fn edge_indices_are_lazy_cloneable_and_thread_safe() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DelaunayTriangulation>();
    let result = fixture(128);
    assert!(result.point_indices.get().is_none());
    assert!(result.clone().point_indices.get().is_none());
    let debug_before = format!("{result:?}");
    let addresses = std::thread::scope(|scope| {
        (0..4).map(|_| scope.spawn(|| {
            let edges = result.edges();
            assert_eq!(signature(&edges), signature(&result.edges_reference()));
            result.point_indices().as_ptr() as usize
        })).collect::<Vec<_>>().into_iter().map(|handle| handle.join().unwrap()).collect::<Vec<_>>()
    });
    assert!(addresses.iter().all(|&address| address == addresses[0]));
    assert_eq!(debug_before, format!("{result:?}"));
    let cloned = result.clone();
    assert_eq!(result.point_indices(), cloned.point_indices());
    verify(&cloned);
    let empty = DelaunayTriangulation::new(Vec::new(), result.points.clone());
    assert!(empty.edges().is_empty());
    assert!(empty.point_indices().is_empty());
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b11_delaunay_complete() {
    for count in [16, 64, 128] {
        let mut state = 159837u64;
        let points = (0..count).map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (state >> 32) as f64 / u32::MAX as f64;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            Point2::new(x, (state >> 32) as f64 / u32::MAX as f64)
        }).collect::<Vec<_>>();
        for with_edges in [false, true] {
            for sample in 0..5 {
                for reference in if sample % 2 == 0 { [true, false] } else { [false, true] } {
                    let start = std::time::Instant::now();
                    let result = if reference { triangulate_reference(std::hint::black_box(&points)) }
                        else { delaunay_triangulate(std::hint::black_box(&points)) }.unwrap();
                    let edges = if with_edges {
                        if reference { result.edges_reference() } else { result.edges() }
                    } else { Vec::new() };
                    let elapsed = start.elapsed();
                    eprintln!("delaunay_complete count={count} with_edges={with_edges} reference={reference} sample={sample} elapsed={elapsed:?}");
                    std::hint::black_box((result, edges));
                }
            }
        }
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b11_delaunay_edges() {
    for count in [64, 256, 1024] {
        for cold in [true, false] {
            for sample in 0..5 {
                let result = fixture(count);
                if !cold { std::hint::black_box(result.edges()); }
                let start = std::time::Instant::now();
                let edges = std::hint::black_box(&result).edges();
                let elapsed = start.elapsed();
                eprintln!("delaunay_edges count={count} cold={cold} sample={sample} elapsed={elapsed:?} results={}", edges.len());
                std::hint::black_box(edges);
            }
        }
    }
}

impl DelaunayTriangulation {
    fn edges_reference(&self) -> Vec<Edge2> {
        let mut edges: Vec<Edge2> = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();

        for triangle in &self.triangles {
            for edge in triangle.edges() {
                let (p1, p2) = edge.into_points();
                let (i1, i2) = self.find_point_indices_reference(&p1, &p2);
                let key = if i1 < i2 { (i1, i2) } else { (i2, i1) };

                if !seen.contains(&key) {
                    seen.insert(key);
                    edges.push(Edge::new(p1, p2));
                }
            }
        }

        edges
    }

    /// 查找点在原始点集中的索引
    /// Find indices of points in the original point set
    fn find_point_indices_reference(&self, p1: &Point2, p2: &Point2) -> (usize, usize) {
        let mut i1 = 0;
        let mut i2 = 0;

        for (i, p) in self.points.iter().enumerate() {
            if p.approx_eq(p1) {
                i1 = i;
            }
            if p.approx_eq(p2) {
                i2 = i;
            }
        }

        (i1, i2)
    }
}

fn triangulate_reference(points: &[Point2]) -> Result<DelaunayTriangulation, String> {
    if points.len() < 3 {
        return Err("At least 3 points are required for triangulation".to_string());
    }

    // 创建超级三角形 / Create super triangle
    let super_triangle = create_super_triangle(points)?;

    // 初始化三角形列表 / Initialize triangle list
    let mut triangles: Vec<Triangle2> = vec![super_triangle.clone()];

    // 逐点插入 / Insert points one by one
    for point in points {
        insert_point(&mut triangles, point);
    }

    // 移除与超级三角形相关的三角形 / Remove triangles connected to super triangle
    let super_vertices: Vec<&Point2> = super_triangle.vertices().to_vec();
    triangles.retain(|t| {
        !t.vertices()
            .iter()
            .any(|v| super_vertices.iter().any(|sv| v.approx_eq(sv)))
    });

    Ok(DelaunayTriangulation {
        triangles,
        point_indices: OnceLock::new(),
        points: points.to_vec(),
    })
}
