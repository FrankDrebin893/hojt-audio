"""Self-contained NumPy CMA-ES optimizer with box-bound projection."""

from __future__ import annotations

import numpy as np


class CMAES:
    """Covariance Matrix Adaptation Evolution Strategy on [0, 1]^d (mapped to bounds)."""

    def __init__(
        self,
        x0: np.ndarray,
        bounds: list[tuple[float, float]],
        sigma0: float = 0.2,
        popsize: int | None = None,
        seed: int = 42,
    ) -> None:
        self.bounds = np.asarray(bounds, dtype=np.float64)
        self.dim = len(bounds)
        self.rng = np.random.default_rng(seed)

        lo, hi = self.bounds[:, 0], self.bounds[:, 1]
        self.span = np.maximum(hi - lo, 1e-12)
        self.mean = np.clip((np.asarray(x0, dtype=np.float64) - lo) / self.span, 0.0, 1.0)
        self.sigma = float(sigma0)

        self.lam = popsize if popsize is not None else max(6, 4 + int(3 * np.log(self.dim)))
        self.mu = self.lam // 2
        raw_w = np.log(self.mu + 0.5) - np.log(np.arange(1, self.mu + 1))
        self.weights = raw_w / np.sum(raw_w)
        self.mu_eff = 1.0 / float(np.sum(self.weights ** 2))

        n = float(self.dim)
        self.cc = (4.0 + self.mu_eff / n) / (n + 4.0 + 2.0 * self.mu_eff / n)
        self.cs = (self.mu_eff + 2.0) / (n + self.mu_eff + 5.0)
        self.c1 = 2.0 / ((n + 1.3) ** 2 + self.mu_eff)
        self.cmu = min(
            1.0 - self.c1,
            2.0 * (self.mu_eff - 2.0 + 1.0 / self.mu_eff) / ((n + 2.0) ** 2 + self.mu_eff),
        )
        self.damps = 1.0 + 2.0 * max(0.0, np.sqrt((self.mu_eff - 1.0) / (n + 1.0)) - 1.0) + self.cs
        self.chi_n = np.sqrt(n) * (1.0 - 1.0 / (4.0 * n) + 1.0 / (21.0 * n * n))

        self.pc = np.zeros(self.dim, dtype=np.float64)
        self.ps = np.zeros(self.dim, dtype=np.float64)
        self.C = np.eye(self.dim, dtype=np.float64)
        self.B = np.eye(self.dim, dtype=np.float64)
        self.D = np.ones(self.dim, dtype=np.float64)
        self.invsqrtC = np.eye(self.dim, dtype=np.float64)
        self.gen = 0

    def ask(self) -> list[np.ndarray]:
        """Sample `lam` candidate vectors in real parameter space."""
        lo = self.bounds[:, 0]
        candidates: list[np.ndarray] = []
        for _ in range(self.lam):
            z = self.rng.standard_normal(self.dim)
            y = self.B @ (self.D * z)
            u = np.clip(self.mean + self.sigma * y, 0.0, 1.0)
            candidates.append(lo + u * self.span)
        return candidates

    def tell(self, xs: list[np.ndarray], fs: list[float]) -> None:
        """Update distribution from evaluated `(x, loss)` pairs."""
        lo = self.bounds[:, 0]
        order = np.argsort(fs)
        us = np.array([(np.asarray(xs[i]) - lo) / self.span for i in order[: self.mu]])

        old_mean = self.mean.copy()
        self.mean = np.sum(self.weights[:, None] * us, axis=0)

        y_w = (self.mean - old_mean) / max(self.sigma, 1e-12)
        self.ps = (1.0 - self.cs) * self.ps + np.sqrt(self.cs * (2.0 - self.cs) * self.mu_eff) * (
            self.invsqrtC @ y_w
        )
        ps_norm = float(np.linalg.norm(self.ps))
        hsig = float(
            ps_norm / np.sqrt(max(1e-12, 1.0 - (1.0 - self.cs) ** (2 * (self.gen + 1))))
            < (1.4 + 2.0 / (self.dim + 1.0)) * self.chi_n
        )

        self.pc = (1.0 - self.cc) * self.pc + hsig * np.sqrt(
            self.cc * (2.0 - self.cc) * self.mu_eff
        ) * y_w

        artmp = (us - old_mean) / max(self.sigma, 1e-12)
        rank_mu = (artmp.T * self.weights) @ artmp
        self.C = (
            (1.0 - self.c1 - self.cmu) * self.C
            + self.c1 * (np.outer(self.pc, self.pc) + (1.0 - hsig) * self.cc * (2.0 - self.cc) * self.C)
            + self.cmu * rank_mu
        )
        self.C = 0.5 * (self.C + self.C.T)

        self.sigma *= float(np.exp((self.cs / self.damps) * (ps_norm / self.chi_n - 1.0)))
        self.sigma = float(np.clip(self.sigma, 1e-4, 0.5))

        eigvals, eigvecs = np.linalg.eigh(self.C)
        eigvals = np.maximum(eigvals, 1e-12)
        self.D = np.sqrt(eigvals)
        self.B = eigvecs
        self.invsqrtC = eigvecs @ np.diag(1.0 / self.D) @ eigvecs.T
        self.gen += 1
