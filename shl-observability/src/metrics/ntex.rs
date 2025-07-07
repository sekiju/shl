use ntex::{Middleware, Service, ServiceCtx, web};
use std::time::Instant;

pub struct PrometheusMiddlewareService<S> {
    service: S,
}

impl<S, Err> Service<web::WebRequest<Err>> for PrometheusMiddlewareService<S>
where
    S: Service<web::WebRequest<Err>, Response = web::WebResponse, Error = web::Error>,
    Err: web::ErrorRenderer,
{
    type Response = web::WebResponse;
    type Error = web::Error;

    ntex::forward_ready!(service);

    async fn call(&self, req: web::WebRequest<Err>, ctx: ServiceCtx<'_, Self>) -> Result<Self::Response, Self::Error> {
        let start = Instant::now();
        let path = req.path().to_string();
        let method = req.method().clone();

        let res = ctx.call(&self.service, req).await?;

        let latency = start.elapsed().as_secs_f64();
        let status = res.status().as_u16().to_string();

        let labels = [("method", method.to_string()), ("path", path), ("status", status)];

        metrics::counter!("http_requests_total", &labels).increment(1);
        metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

        Ok(res)
    }
}

pub struct PrometheusMiddleware;

impl<S> Middleware<S> for PrometheusMiddleware {
    type Service = PrometheusMiddlewareService<S>;

    fn create(&self, service: S) -> Self::Service {
        PrometheusMiddlewareService { service }
    }
}

impl Default for PrometheusMiddleware {
    fn default() -> Self {
        Self {}
    }
}
