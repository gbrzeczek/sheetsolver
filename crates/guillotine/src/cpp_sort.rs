//! `std::sort` exactly as the oracle's C++ standard library runs it.
//!
//! `std::sort` is not stable: elements that compare equal end up in an
//! order that depends on the library's algorithm. Upstream sorts candidate
//! moves with comparators that often tie (two blocks of the same value),
//! and the order of tied moves decides which one the search tries first.
//! To make the same decisions, this module reproduces libc++'s algorithm
//! (LLVM's libc++ as shipped with Apple clang 21, `__algorithm/sort.h`,
//! `__introsort` without bitset partitioning, which libc++ only uses for
//! arithmetic types), down to every comparison and swap.
//!
//! `less(a, b)` must be a strict weak order, as for `std::sort`.

/// Sorts `v` the way libc++'s `std::sort(v.begin(), v.end(), less)` does.
pub(crate) fn sort<T: Copy>(v: &mut [T], less: impl Fn(&T, &T) -> bool) {
    if v.is_empty() {
        return;
    }
    // upstream libc++: `2 * __bit_log2(n)`.
    let depth = 2 * (usize::BITS - 1 - v.len().leading_zeros()) as usize;
    introsort(v, 0, v.len(), &less, depth, true);
}

/// libc++ `__sort3`: sorts three elements, returns whether it swapped.
fn sort3<T: Copy>(v: &mut [T], x: usize, y: usize, z: usize, less: &impl Fn(&T, &T) -> bool) {
    if !less(&v[y], &v[x]) {
        if !less(&v[z], &v[y]) {
            return;
        }
        v.swap(y, z);
        if less(&v[y], &v[x]) {
            v.swap(x, y);
        }
        return;
    }
    if less(&v[z], &v[y]) {
        v.swap(x, z);
        return;
    }
    v.swap(x, y);
    if less(&v[z], &v[y]) {
        v.swap(y, z);
    }
}

/// libc++ `__sort4`.
fn sort4<T: Copy>(v: &mut [T], x: [usize; 4], less: &impl Fn(&T, &T) -> bool) {
    sort3(v, x[0], x[1], x[2], less);
    if less(&v[x[3]], &v[x[2]]) {
        v.swap(x[2], x[3]);
        if less(&v[x[2]], &v[x[1]]) {
            v.swap(x[1], x[2]);
            if less(&v[x[1]], &v[x[0]]) {
                v.swap(x[0], x[1]);
            }
        }
    }
}

/// libc++ `__sort5`.
fn sort5<T: Copy>(v: &mut [T], x: [usize; 5], less: &impl Fn(&T, &T) -> bool) {
    sort4(v, [x[0], x[1], x[2], x[3]], less);
    if less(&v[x[4]], &v[x[3]]) {
        v.swap(x[3], x[4]);
        if less(&v[x[3]], &v[x[2]]) {
            v.swap(x[2], x[3]);
            if less(&v[x[2]], &v[x[1]]) {
                v.swap(x[1], x[2]);
                if less(&v[x[1]], &v[x[0]]) {
                    v.swap(x[0], x[1]);
                }
            }
        }
    }
}

/// libc++ `__insertion_sort` on `[first, last)`; with `guarded` false,
/// `__insertion_sort_unguarded`, which relies on the element before
/// `first` being no greater than any element of the range.
fn insertion_sort<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
    guarded: bool,
) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        let mut j = i - 1;
        if less(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if guarded && j == first {
                    break;
                }
                k -= 1;
                if !less(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
        }
    }
}

/// libc++ `__insertion_sort_incomplete`: insertion sort that gives up
/// after 8 moves. Returns whether the range is sorted.
fn insertion_sort_incomplete<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> bool {
    match last - first {
        0 | 1 => return true,
        2 => {
            if less(&v[last - 1], &v[first]) {
                v.swap(first, last - 1);
            }
            return true;
        }
        3 => {
            sort3(v, first, first + 1, last - 1, less);
            return true;
        }
        4 => {
            sort4(v, [first, first + 1, first + 2, last - 1], less);
            return true;
        }
        5 => {
            sort5(v, [first, first + 1, first + 2, first + 3, last - 1], less);
            return true;
        }
        _ => {}
    }
    let mut j = first + 2;
    sort3(v, first, first + 1, j, less);
    let mut count = 0;
    let mut i = j + 1;
    while i != last {
        if less(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !less(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
            count += 1;
            if count == 8 {
                return i + 1 == last;
            }
        }
        j = i;
        i += 1;
    }
    true
}

/// libc++ `__partition_with_equals_on_right`, around the pivot at
/// `first`. Returns the pivot's position and whether the range was
/// already partitioned.
fn partition_with_equals_on_right<T: Copy>(
    v: &mut [T],
    begin: usize,
    end: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> (usize, bool) {
    let pivot = v[begin];
    let mut first = begin;
    loop {
        first += 1;
        if !less(&v[first], &pivot) {
            break;
        }
    }
    let mut last = end;
    if begin == first - 1 {
        while first < last {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let already_partitioned = first >= last;
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if !less(&v[first], &pivot) {
                break;
            }
        }
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    if begin != pivot_pos {
        v[begin] = v[pivot_pos];
    }
    v[pivot_pos] = pivot;
    (pivot_pos, already_partitioned)
}

/// libc++ `__partition_with_equals_on_left`. Returns the first position
/// after the elements equal to the pivot.
fn partition_with_equals_on_left<T: Copy>(
    v: &mut [T],
    begin: usize,
    end: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    if less(&pivot, &v[last - 1]) {
        loop {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
    } else {
        loop {
            first += 1;
            if first >= last || less(&pivot, &v[first]) {
                break;
            }
        }
    }
    if first < last {
        loop {
            last -= 1;
            if !less(&pivot, &v[last]) {
                break;
            }
        }
    }
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
        loop {
            last -= 1;
            if !less(&pivot, &v[last]) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    if begin != pivot_pos {
        v[begin] = v[pivot_pos];
    }
    v[pivot_pos] = pivot;
    first
}

/// libc++ `__introsort`.
fn introsort<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    less: &impl Fn(&T, &T) -> bool,
    mut depth: usize,
    mut leftmost: bool,
) {
    const LIMIT: usize = 24;
    const NINTHER_THRESHOLD: usize = 128;
    loop {
        let len = last - first;
        match len {
            0 | 1 => return,
            2 => {
                if less(&v[last - 1], &v[first]) {
                    v.swap(first, last - 1);
                }
                return;
            }
            3 => {
                sort3(v, first, first + 1, last - 1, less);
                return;
            }
            4 => {
                sort4(v, [first, first + 1, first + 2, last - 1], less);
                return;
            }
            5 => {
                sort5(v, [first, first + 1, first + 2, first + 3, last - 1], less);
                return;
            }
            _ => {}
        }
        if len < LIMIT {
            insertion_sort(v, first, last, less, leftmost);
            return;
        }
        if depth == 0 {
            heap_sort(v, first, last, less);
            return;
        }
        depth -= 1;
        let half = len / 2;
        if len > NINTHER_THRESHOLD {
            sort3(v, first, first + half, last - 1, less);
            sort3(v, first + 1, first + half - 1, last - 2, less);
            sort3(v, first + 2, first + half + 1, last - 3, less);
            sort3(v, first + half - 1, first + half, first + half + 1, less);
            v.swap(first, first + half);
        } else {
            sort3(v, first + half, first, last - 1, less);
        }
        if !leftmost && !less(&v[first - 1], &v[first]) {
            first = partition_with_equals_on_left(v, first, last, less);
            continue;
        }
        let (i, already_partitioned) = partition_with_equals_on_right(v, first, last, less);
        if already_partitioned {
            let left_sorted = insertion_sort_incomplete(v, first, i, less);
            if insertion_sort_incomplete(v, i + 1, last, less) {
                if left_sorted {
                    return;
                }
                last = i;
                continue;
            } else if left_sorted {
                first = i + 1;
                continue;
            }
        }
        introsort(v, first, i, less, depth, leftmost);
        leftmost = false;
        first = i + 1;
    }
}

/// libc++ `__partial_sort(first, last, last)`: heap sort, introsort's
/// fallback.
fn heap_sort<T: Copy>(v: &mut [T], first: usize, last: usize, less: &impl Fn(&T, &T) -> bool) {
    let len = last - first;
    // `__make_heap`.
    if len > 1 {
        for start in (0..=(len - 2) / 2).rev() {
            sift_down(v, first, len, first + start, less);
        }
    }
    // `__sort_heap`.
    let mut end = last;
    let mut n = len;
    while n > 1 {
        pop_heap(v, first, end, n, less);
        end -= 1;
        n -= 1;
    }
}

/// libc++ `__sift_down` of the heap at `first` of length `len`.
fn sift_down<T: Copy>(
    v: &mut [T],
    first: usize,
    len: usize,
    mut start: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    let mut child = start - first;
    if len < 2 || (len - 2) / 2 < child {
        return;
    }
    child = 2 * child + 1;
    let mut child_i = first + child;
    if child + 1 < len && less(&v[child_i], &v[child_i + 1]) {
        child_i += 1;
        child += 1;
    }
    if less(&v[child_i], &v[start]) {
        return;
    }
    let top = v[start];
    loop {
        v[start] = v[child_i];
        start = child_i;
        if (len - 2) / 2 < child {
            break;
        }
        child = 2 * child + 1;
        child_i = first + child;
        if child + 1 < len && less(&v[child_i], &v[child_i + 1]) {
            child_i += 1;
            child += 1;
        }
        if less(&v[child_i], &top) {
            break;
        }
    }
    v[start] = top;
}

/// libc++ `__pop_heap` with `__floyd_sift_down` and `__sift_up`.
fn pop_heap<T: Copy>(
    v: &mut [T],
    first: usize,
    mut last: usize,
    len: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    if len <= 1 {
        return;
    }
    let top = v[first];
    // `__floyd_sift_down`.
    let mut hole = first;
    let mut child_i = first;
    let mut child = 0;
    loop {
        child_i += child + 1;
        child = 2 * child + 1;
        if child + 1 < len && less(&v[child_i], &v[child_i + 1]) {
            child_i += 1;
            child += 1;
        }
        v[hole] = v[child_i];
        hole = child_i;
        if child > (len - 2) / 2 {
            break;
        }
    }
    last -= 1;
    if hole == last {
        v[hole] = top;
    } else {
        v[hole] = v[last];
        hole += 1;
        v[last] = top;
        // `__sift_up(first, hole, len = hole - first)`.
        let mut up_len = hole - first;
        if up_len > 1 {
            up_len = (up_len - 2) / 2;
            let mut ptr = first + up_len;
            let mut end = hole - 1;
            if less(&v[ptr], &v[end]) {
                let t = v[end];
                loop {
                    v[end] = v[ptr];
                    end = ptr;
                    if up_len == 0 {
                        break;
                    }
                    up_len = (up_len - 1) / 2;
                    ptr = first + up_len;
                    if !less(&v[ptr], &t) {
                        break;
                    }
                }
                v[end] = t;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sort;

    /// A small deterministic generator, so the tests need no crate.
    fn values(seed: u64, count: usize, range: u64) -> Vec<(u64, usize)> {
        let mut state = seed;
        (0..count)
            .map(|index| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 33) % range, index)
            })
            .collect()
    }

    #[test]
    fn sorts_every_length_with_many_ties() {
        for count in 0..400 {
            for range in [1, 3, 50, 1000] {
                let mut v = values(count as u64 * 7 + range, count, range);
                sort(&mut v, |a, b| a.0 < b.0);
                assert!(v.windows(2).all(|w| w[0].0 <= w[1].0), "{count} {range}");
                let mut indices: Vec<usize> = v.iter().map(|x| x.1).collect();
                indices.sort();
                assert!(indices.iter().copied().eq(0..count));
            }
        }
    }

    #[test]
    fn small_ranges_keep_ties_in_order() {
        // Below 24 elements libc++ uses insertion sort, which is stable.
        let mut v = values(5, 20, 3);
        sort(&mut v, |a, b| a.0 < b.0);
        assert!(v.windows(2).all(|w| w[0].0 < w[1].0 || w[0].1 < w[1].1));
    }
}
