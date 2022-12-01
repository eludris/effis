use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

use rocket::{http::Header, response::Responder};
use rocket_db_pools::{deadpool_redis::redis::AsyncCommands, Connection};
use todel::{
    models::{ErrorResponse, ErrorResponseData, FileSizeRatelimitedError, RatelimitError},
    Conf,
};

use crate::Cache;

pub type RatelimitedRouteResponse<T> =
    Result<RatelimitHeaderWrapper<T>, RatelimitHeaderWrapper<ErrorResponse>>;

/// The necessary headers for responses
#[derive(Debug, Responder)]
pub struct RatelimitHeaderWrapper<T> {
    pub inner: T,
    pub ratelimit_reset: Header<'static>,
    pub ratelimit_max: Header<'static>,
    pub ratelimit_bytes_limit: Header<'static>,
    pub ratelimit_last_reset: Header<'static>,
    pub ratelimit_request_count: Header<'static>,
    pub ratelimit_sent_bytes: Header<'static>,
}

#[derive(Debug)]
pub struct Ratelimiter {
    key: String,
    reset_after: Duration,
    request_limit: u32,
    file_size_limit: u64,
    request_count: u32,
    last_reset: u64,
    sent_bytes: u64,
}

impl Ratelimiter {
    /// Creates a new Ratelimiter
    pub fn new<I>(bucket: &str, attachment_bucket: &str, identifier: I, conf: &Conf) -> Ratelimiter
    where
        I: Display,
    {
        let (reset_after, request_limit, file_size_limit) = match bucket {
            "assets" => (
                &conf.effis.ratelimits.assets.reset_after,
                &conf.effis.ratelimits.assets.limit,
                conf.effis.ratelimits.assets.file_size_limit,
            ),
            "attachments" => (
                &conf.effis.ratelimits.attachments.reset_after,
                &conf.effis.ratelimits.attachments.limit,
                conf.effis.ratelimits.attachments.file_size_limit,
            ),
            "fetch_file" => (
                &conf.effis.ratelimits.fetch_file.reset_after,
                &conf.effis.ratelimits.fetch_file.limit,
                0,
            ),

            _ => unreachable!(),
        };
        Self {
            key: format!("ratelimit:{}:{}-{}", identifier, bucket, attachment_bucket),
            reset_after: Duration::from_secs(*reset_after as u64),
            request_limit: *request_limit,
            file_size_limit,
            request_count: 0,
            last_reset: 0,
            sent_bytes: 0,
        }
    }

    /// Checks if a bucket is ratelimited, if so returns an Error with an ErrorResponse
    pub async fn process_ratelimit(
        &mut self,
        bytes: u64,
        cache: &mut Connection<Cache>,
    ) -> Result<(), RatelimitHeaderWrapper<ErrorResponse>> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;

        if bytes > self.file_size_limit {
            return Err(self
                .wrap_response::<_, ()>(
                    FileSizeRatelimitedError {
                        retry_after: self.last_reset + self.reset_after.as_millis() as u64 - now,
                        bytes_left: self.file_size_limit - self.sent_bytes,
                    }
                    .to_error_response(),
                )
                .unwrap());
        }

        if let (Some(last_reset), Some(request_count), Some(sent_bytes)) = cache
            .hget::<&str, (&str, &str, &str), (Option<u64>, Option<u32>, Option<u64>)>(
                &self.key,
                ("last_reset", "request_count", "sent_bytes"),
            )
            .await
            .expect("Couldn't query cache")
        {
            self.last_reset = last_reset;
            self.request_count = request_count;
            self.sent_bytes = sent_bytes;
            if now - self.last_reset >= self.reset_after.as_millis() as u64 {
                cache
                    .del::<&str, ()>(&self.key)
                    .await
                    .expect("Couldn't query cache");
                cache
                    .hset_multiple::<&str, &str, u64, ()>(
                        &self.key,
                        &[("last_reset", now), ("request_count", 0)],
                    )
                    .await
                    .expect("Couldn't query cache");
                self.last_reset = now;
                self.request_count = 0;
                self.sent_bytes = 0;
                log::debug!("Reset bucket for {}", self.key);
            }
            if self.request_count >= self.request_limit {
                log::info!("Ratelimited bucket {}", self.key);
                Err(self
                    .wrap_response::<_, ()>(
                        RatelimitError {
                            retry_after: self.last_reset + self.reset_after.as_millis() as u64
                                - now,
                        }
                        .to_error_response(),
                    )
                    .unwrap())
            } else if self.sent_bytes + bytes > self.file_size_limit {
                Err(self
                    .wrap_response::<_, ()>(
                        FileSizeRatelimitedError {
                            retry_after: self.last_reset + self.reset_after.as_millis() as u64
                                - now,
                            bytes_left: self.file_size_limit - self.sent_bytes,
                        }
                        .to_error_response(),
                    )
                    .unwrap())
            } else {
                cache
                    .hincr::<&str, &str, u8, ()>(&self.key, "request_count", 1)
                    .await
                    .expect("Couldn't query cache");
                self.request_count += 1;
                cache
                    .hincr::<&str, &str, u64, ()>(&self.key, "sent_bytes", bytes)
                    .await
                    .expect("Couldn't query cache");
                self.sent_bytes += bytes;
                Ok(())
            }
        } else {
            log::debug!("New bucket for {}", self.key);
            cache
                .hset_multiple::<&str, &str, u64, ()>(
                    &self.key,
                    &[
                        ("last_reset", now),
                        ("request_count", 1),
                        ("sent_bytes", bytes),
                    ],
                )
                .await
                .expect("Couldn't query cache");
            Ok(())
        }
    }

    pub fn wrap_response<T, E>(&self, data: T) -> Result<RatelimitHeaderWrapper<T>, E> {
        Ok(RatelimitHeaderWrapper {
            inner: data,
            ratelimit_reset: Header::new(
                "X-Ratelimit-Reset",
                self.reset_after.as_millis().to_string(),
            ),
            ratelimit_max: Header::new("X-Ratelimit-Max", self.request_limit.to_string()),
            ratelimit_bytes_limit: Header::new(
                "X-Ratelimit-Bytes-Left",
                self.file_size_limit.to_string(),
            ),
            ratelimit_last_reset: Header::new(
                "X-Ratelimit-Last-Reset",
                self.last_reset.to_string(),
            ),
            ratelimit_request_count: Header::new(
                "X-Ratelimit-Request-Count",
                self.request_count.to_string(),
            ),
            ratelimit_sent_bytes: Header::new(
                "X-Ratelimit-Sent-Bytes",
                self.sent_bytes.to_string(),
            ),
        })
    }
}
