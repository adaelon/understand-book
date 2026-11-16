"""两道选做编程题：LASSO 与 Gauss–Newton。依赖 NumPy。"""

from pathlib import Path
import numpy as np


def soft_threshold(x, threshold):
    return np.sign(x) * np.maximum(np.abs(x) - threshold, 0.0)


def lasso(A, b, mu, x0=None, tol=1e-8, max_iter=20000):
    """最小化 0.5*||Ax-b||^2 + mu*||x||_1。"""
    A, b = np.asarray(A, dtype=float), np.asarray(b, dtype=float)
    if A.ndim != 2 or b.shape != (A.shape[0],):
        raise ValueError("A 应为矩阵，b 的长度应等于 A 的行数。")
    if mu < 0 or tol <= 0 or max_iter < 1:
        raise ValueError("要求 mu>=0、tol>0、max_iter>=1。")
    x = np.zeros(A.shape[1]) if x0 is None else np.array(x0, dtype=float, copy=True)
    if x.shape != (A.shape[1],):
        raise ValueError("x0 的长度应等于 A 的列数。")
    lipschitz = np.linalg.norm(A, 2) ** 2
    if lipschitz == 0:
        return dict(x=np.zeros_like(x), converged=True, iterations=0,
                    stationarity=0.0, reason="观测矩阵为零，零向量最优。")
    step = 1.0 / lipschitz
    for k in range(max_iter):
        new = soft_threshold(x - step * (A.T @ (A @ x - b)), step * mu)
        stationarity = np.linalg.norm((x - new) / step)
        x = new
        if stationarity <= tol:
            return dict(x=x, converged=True, iterations=k + 1,
                        stationarity=float(stationarity), reason="近端梯度映射达到容差。")
    return dict(x=x, converged=False, iterations=max_iter,
                stationarity=float(stationarity), reason="达到迭代上限。")


def lasso_path(A, b, mus, zero_tol=1e-7, **kwargs):
    """固定数据，分别求解各个参数；同时记录收敛状态。"""
    A, b = np.asarray(A, dtype=float), np.asarray(b, dtype=float)
    rows = []
    for mu in mus:
        result = lasso(A, b, float(mu), **kwargs)
        x = result["x"]
        result.update(mu=float(mu), residual=float(np.linalg.norm(A @ x - b)),
                      l1=float(np.linalg.norm(x, 1)),
                      nonzero=int(np.count_nonzero(np.abs(x) > zero_tol)))
        rows.append(result)
    return rows


def gauss_newton(residual, jacobian, x0, tol=1e-8, max_iter=200):
    """列满秩 QR 高斯–牛顿方向，配 Armijo 回溯。"""
    x = np.array(x0, dtype=float, copy=True)
    if x.ndim != 1 or tol <= 0 or max_iter < 1:
        raise ValueError("x0 应为向量，要求 tol>0、max_iter>=1。")

    def finish(k, ok, reason, r, g):
        return dict(x=x.copy(), converged=ok, iterations=k, reason=reason,
                    objective=float(0.5 * (r @ r)), gradient_norm=float(np.linalg.norm(g)))

    for k in range(max_iter + 1):
        r = np.asarray(residual(x), dtype=float)
        J = np.asarray(jacobian(x), dtype=float)
        if r.ndim != 1 or J.shape != (r.size, x.size):
            raise ValueError("残量和雅可比矩阵维数不匹配。")
        if not np.isfinite(r).all() or not np.isfinite(J).all():
            raise ValueError("当前点的残量或雅可比矩阵不是有限值。")
        g = J.T @ r
        if np.linalg.norm(g) <= tol:
            return finish(k, True, "梯度范数达到容差。", r, g)
        if k == max_iter:
            return finish(k, False, "达到迭代上限。", r, g)
        if np.linalg.matrix_rank(J) < x.size:
            return finish(k, False, "雅可比矩阵不列满秩，可改用 LM 方法。", r, g)
        Q, R = np.linalg.qr(J, mode="reduced")
        direction = np.linalg.solve(R, -(Q.T @ r))
        slope = float(g @ direction)
        if slope >= 0:
            return finish(k, False, "数值计算未得到下降方向。", r, g)
        value = float(0.5 * (r @ r))
        step = 1.0
        accepted = False
        for _ in range(60):
            trial = x + step * direction
            with np.errstate(over="ignore", invalid="ignore"):
                rr = np.asarray(residual(trial), dtype=float)
                trial_value = float(0.5 * (rr @ rr))
            if np.isfinite(trial_value) and trial_value <= value + 1e-4 * step * slope:
                accepted = True
                break
            step *= 0.5
        if not accepted:
            return finish(k, False, "回溯未找到可接受步长。", r, g)
        x = trial


def chlorine_problem():
    """氯气含量模型与 44 组观测。"""
    data = np.loadtxt(Path(__file__).with_name("chlorine-data.csv"),
                      delimiter=",", skiprows=1)
    time, observed = data[:, 0] - 8.0, data[:, 1]

    def residual(x):
        alpha, beta = x
        return alpha + (0.49 - alpha) * np.exp(-beta * time) - observed

    def jacobian(x):
        alpha, beta = x
        exp = np.exp(-beta * time)
        return np.column_stack((1 - exp, -(0.49 - alpha) * time * exp))

    return residual, jacobian


if __name__ == "__main__":
    r, J = chlorine_problem()
    # 初始点是程序运行设置；模型和观测值按题目给定。
    result = gauss_newton(r, J, [0.4, 0.1])
    print("氯气含量模型")
    print("alpha, beta =", result["x"])
    print("converged =", result["converged"], result["reason"])
    print("iterations =", result["iterations"])
    print("gradient norm =", result["gradient_norm"])
    print("residual sum of squares =", 2 * result["objective"])
