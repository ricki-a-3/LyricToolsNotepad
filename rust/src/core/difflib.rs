//! A port of the part of Python's `difflib.SequenceMatcher` that near-rhyme
//! scoring uses: `ratio()` over two short phoneme sequences. Kept exact so
//! scores match the original plugin to the last digit.
//!
//! No junk heuristics: `isjunk` is None in the original, and `autojunk` only
//! applies to sequences of 200+ items (rhyme tails are a handful of phones).

use std::collections::HashMap;

/// `SequenceMatcher(None, a, b).ratio()`
pub fn ratio<T: Eq + std::hash::Hash + Copy>(a: &[T], b: &[T]) -> f64 {
    let total = a.len() + b.len();
    if total == 0 {
        return 1.0;
    }
    2.0 * matching_count(a, b) as f64 / total as f64
}

/// Total size of `get_matching_blocks()`.
fn matching_count<T: Eq + std::hash::Hash + Copy>(a: &[T], b: &[T]) -> usize {
    let mut b2j: HashMap<T, Vec<usize>> = HashMap::new();
    for (j, elt) in b.iter().enumerate() {
        b2j.entry(*elt).or_default().push(j);
    }
    let mut total = 0;
    let mut queue = vec![(0, a.len(), 0, b.len())];
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = find_longest_match(a, b, &b2j, alo, ahi, blo, bhi);
        if k > 0 {
            total += k;
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    total
}

fn find_longest_match<T: Eq + std::hash::Hash + Copy>(
    a: &[T],
    b: &[T],
    b2j: &HashMap<T, Vec<usize>>,
    alo: usize,
    ahi: usize,
    blo: usize,
    bhi: usize,
) -> (usize, usize, usize) {
    let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0);
    // j2len[j + 1] = length of the match ending at a[i-1], b[j]
    let mut j2len = vec![0usize; b.len() + 1];
    let mut newj2len = vec![0usize; b.len() + 1];
    for i in alo..ahi {
        let mut touched = Vec::new();
        if let Some(js) = b2j.get(&a[i]) {
            for &j in js {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let k = j2len[j] + 1;
                newj2len[j + 1] = k;
                touched.push(j + 1);
                if k > bestsize {
                    besti = i + 1 - k;
                    bestj = j + 1 - k;
                    bestsize = k;
                }
            }
        }
        j2len.iter_mut().for_each(|v| *v = 0);
        for t in touched {
            j2len[t] = newj2len[t];
            newj2len[t] = 0;
        }
    }
    while besti > alo && bestj > blo && a[besti - 1] == b[bestj - 1] {
        besti -= 1;
        bestj -= 1;
        bestsize += 1;
    }
    while besti + bestsize < ahi && bestj + bestsize < bhi && a[besti + bestsize] == b[bestj + bestsize] {
        bestsize += 1;
    }
    (besti, bestj, bestsize)
}

#[cfg(test)]
mod tests {
    use super::ratio;

    #[test]
    fn matches_python_difflib() {
        // Values from Python: SequenceMatcher(None, a, b).ratio()
        assert_eq!(ratio(b"abcd", b"bcde"), 0.75);
        assert_eq!(ratio(b"AY T", b"AY T"), 1.0);
        assert_eq!(ratio::<u8>(b"", b""), 1.0);
        assert!((ratio(b"abxcd", b"abcd") - 0.8888888888888888).abs() < 1e-15);
        assert_eq!(ratio(b"aaab", b"abbb"), 0.5);
    }
}
