use std::{
    collections::BTreeMap,
    ops::Bound::{Included, Excluded}
};

pub fn remove_span(
    mut sbeg: u64,
    send: u64,
    map: &mut BTreeMap<u64, u64>
)
{
    loop {
        // find span starting not less than sbeg
        let lb = map.range(..=sbeg).last();

        if let Some((&lbeg, &lend)) = lb {
            // lbeg <= sbeg < lend
            if sbeg < lend {
                // remove [sbeg, min(lend, send))

                if lbeg < sbeg {
                    map.insert(lbeg, sbeg);
                }
                else {
                    map.remove(&lbeg);
                }

                if send < lend {
                    map.insert(send, lend);
                    return;
                }
                else if send == lend {
                    return;
                }
                else {
                    sbeg = lend;
                }
            }
        }

        let ub = map.range((Excluded(sbeg), Included(u64::MAX))).next();

        if let Some((&ubeg, &uend)) = ub && ubeg <= send {
            // sbeg < ubeg <= send

            map.remove(&ubeg);

            if send < uend {
                map.insert(send, uend);
                return;
            }
            else {
                sbeg = uend;
            }
        }
        else {
            return;
        }
    }
}

pub fn insert_span<T: Clone + PartialEq>(
    mut sbeg: u64,
    send: u64,
    val: T,
    map: &mut BTreeMap<u64, (u64, T)>
)
{
    loop {
        // find span starting not less than sbeg
        let lb = map.range(..=sbeg).last();

        if let Some((&lbeg, &(lend, ref lv))) = lb {
            if sbeg < lend {
                // lbeg <= sbeg < lend
                if send <= lend {
                    // lbeg <= sbeg < send <= lend
                    // [sbeg, send) is already covered
                    return;
                }
                else {
                    // lbeg <= sbeg <= lend < send
                    // restrict span start to lend
                    sbeg = lend;
                    continue;
                }
            }
            else if sbeg == lend && val == *lv {
                // merge (eventually) with same-valued prev span
                sbeg = lbeg;
            }
        }

        // find next span starting after sbeg
        let ub = map.range((Excluded(sbeg), Included(u64::MAX))).next();

        if let Some((&ubeg, &(uend, ref uv))) = ub && ubeg <= send {
            // sbeg < ubeg <= send

            if *uv == val {
                // merge with same-valued next span
                map.remove(&ubeg);
                map.insert(sbeg, (uend, val.clone()));
            }
            else {
                // insert up to different-valued next span
                map.insert(sbeg, (ubeg, val.clone()));
            };

            if uend < send {
                // resume inserting the rest of span after uend
                sbeg = uend;
            }
            else {
                // we've covered the span to send, so we're done
                return;
            }
        }
        else {
            // sbeg < send < ubeg
            // no overlap with next span, add this span
            map.insert(sbeg, (send, val.clone()));
            return;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_insert_span_one() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(0, 10, 0, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_insert_span_covered_exact_one() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(0, 10, 0, &mut m);
        insert_span(0, 10, 1, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_insert_span_covered_subinterval_one() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(0, 10, 0, &mut m);
        insert_span(1, 9, 1, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_insert_span_covered_subinterval_two() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(0, 5, 0, &mut m);
        insert_span(5, 10, 0, &mut m);
        insert_span(1, 9, 1, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_insert_span_overlap_before() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(4, 10, 0, &mut m);
        insert_span(0, 5, 1, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [
                (0, (4, 1)),
                (4, (10, 0))
            ]
        );
    }

    #[test]
    fn test_insert_span_overlap_middle() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(4, 6, 0, &mut m);
        insert_span(0, 10, 1, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [
                (0, (4, 1)),
                (4, (6, 0)),
                (6, (10, 1))
            ]
        );
    }

    #[test]
    fn test_insert_span_overlap_after() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(0, 5, 0, &mut m);
        insert_span(4, 10, 1, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [
                (0, (5, 0)),
                (5, (10, 1))
            ]
        );
    }

    #[test]
    fn test_insert_span_merge_before() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(5, 10, 0, &mut m);
        insert_span(0, 5, 0, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_insert_span_merge_middle() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(4, 6, 0, &mut m);
        insert_span(0, 10, 0, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_insert_span_merge_after() {
        let mut m: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        insert_span(0, 5, 0, &mut m);
        insert_span(5, 10, 0, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, (10, 0)) ]
        );
    }

    #[test]
    fn test_remove_span_whole() {
        let mut m: BTreeMap<u64, u64> = BTreeMap::from_iter([(0, 10)]);
        remove_span(0, 10, &mut m);
        assert!(m.is_empty());
    }

    #[test]
    fn test_remove_span_empty() {
        let mut m: BTreeMap<u64, u64> = BTreeMap::new();
        remove_span(0, 10, &mut m);
        assert!(m.is_empty());
    }

    #[test]
    fn test_remove_span_overlap_before() {
        let mut m: BTreeMap<u64, u64> = BTreeMap::from([(4, 10)]);
        remove_span(0, 6, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (6, 10) ]
        );
    }

    #[test]
    fn test_remove_span_overlap_middle() {
        let mut m: BTreeMap<u64, u64> = BTreeMap::from([(0, 10)]);
        remove_span(4, 6, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [
                (0, 4),
                (6, 10)
            ]
        );
    }

    #[test]
    fn test_remove_span_overlap_after() {
        let mut m: BTreeMap<u64, u64> = BTreeMap::from([(0, 6)]);
        remove_span(4, 10, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [ (0, 4) ]
        );
    }

    #[test]
    fn test_remove_span_overlap_multi() {
        let mut m: BTreeMap<u64, u64> = BTreeMap::from([(0, 5), (6, 10)]);
        remove_span(4, 7, &mut m);
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [
                (0, 4),
                (7, 10)
            ]
        );
    }
}
