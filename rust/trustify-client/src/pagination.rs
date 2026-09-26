use std::{future::Future, num::NonZeroU64};

/// The items and optional total count returned by one offset-based API page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffsetPage<T> {
    /// Items returned by this page.
    pub items: Vec<T>,
    /// Total number of matching items, when requested and available.
    pub total: Option<u64>,
}

impl<T> OffsetPage<T> {
    /// Construct a page from generated response fields.
    pub fn new(items: Vec<T>, total: Option<u64>) -> Self {
        Self { items, total }
    }
}

/// Fetch all pages from an offset/limit endpoint.
///
/// The callback receives the next `offset` and the fixed `limit`. A known
/// `total` count is authoritative; otherwise a short or empty page ends the
/// traversal. This helper does not retry failed requests.
pub async fn collect_offset_pages<T, E, F, Fut>(
    page_size: NonZeroU64,
    mut fetch_page: F,
) -> Result<Vec<T>, E>
where
    F: FnMut(u64, u64) -> Fut,
    Fut: Future<Output = Result<OffsetPage<T>, E>>,
{
    let limit = page_size.get();
    let mut offset = 0_u64;
    let mut items = Vec::new();

    loop {
        let page = fetch_page(offset, limit).await?;
        let count = u64::try_from(page.items.len()).unwrap_or(u64::MAX);
        let next_offset = offset.saturating_add(count);
        let is_done = match page.total {
            Some(total) => next_offset >= total,
            None => count < limit,
        };

        items.extend(page.items);
        if count == 0 || is_done {
            break;
        }
        offset = next_offset;
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, sync::Mutex};

    use super::{OffsetPage, collect_offset_pages};

    #[test]
    fn follows_offsets_until_the_reported_total_is_collected() {
        let offsets = Mutex::new(Vec::new());
        let items = futures::executor::block_on(collect_offset_pages(
            NonZeroU64::new(2).unwrap(),
            |offset, limit| {
                offsets.lock().unwrap().push((offset, limit));
                async move {
                    let all = ["a", "b", "c", "d", "e"];
                    let page = all
                        .iter()
                        .skip(offset as usize)
                        .take(limit as usize)
                        .copied()
                        .collect();
                    Ok::<_, ()>(OffsetPage::new(page, Some(all.len() as u64)))
                }
            },
        ))
        .unwrap();

        assert_eq!(items, ["a", "b", "c", "d", "e"]);
        assert_eq!(*offsets.lock().unwrap(), [(0, 2), (2, 2), (4, 2)]);
    }

    #[test]
    fn stops_on_an_empty_page_when_the_total_is_unknown() {
        let mut calls = 0;
        let items = futures::executor::block_on(collect_offset_pages(
            NonZeroU64::new(3).unwrap(),
            |_, _| {
                calls += 1;
                async move {
                    Ok::<_, ()>(if calls == 1 {
                        OffsetPage::new(vec![1, 2, 3], None)
                    } else {
                        OffsetPage::new(Vec::<u8>::new(), None)
                    })
                }
            },
        ))
        .unwrap();

        assert_eq!(items, [1, 2, 3]);
        assert_eq!(calls, 2);
    }
}
