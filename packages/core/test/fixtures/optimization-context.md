<!-- Excerpts supplied by the user from 最优化_建模算法与理论.md: theorem 5.3, exercises 2.2/6.3, equation 5.4.14, algorithm 6.2. -->

### 一阶必要条件

定理 5.3 (一阶必要条件) 假设 f 在全空间  $R^n$ 可微. 如果  $x^*$ 是一个局部极小点, 那么

$$
\nabla f(x^{*})=0.
$$

证明. 任取  $v \in \mathbb{R}^n$，考虑  $f$ 在点  $x = x^*$ 处的泰勒展开

$$
\boldsymbol{f}(\boldsymbol{x}^{*}+t\boldsymbol{v})=\boldsymbol{f}(\boldsymbol{x}^{*})+t\boldsymbol{v}^{\mathrm{T}}\nabla\boldsymbol{f}(\boldsymbol{x}^{*})+\boldsymbol{o}(t),
$$

整理得

$$
\frac{f(\boldsymbol{x}^{*}+t\boldsymbol{v})-f(\boldsymbol{x}^{*})}{t}=\boldsymbol{v}^{\mathrm{T}}\nabla f(\boldsymbol{x}^{*})+o(1).
$$

<a id="exercise-2-2"></a>

#### 习题 2.2

证明：矩阵 A 的 2 范数等于其最大奇异值，即

$$
\sigma_{1}(A)=\max_{\|x\|_{2}=1}\|Ax\|_{2}.
$$

<a id="exercise-2-3"></a>

#### 习题 2.3

证明如下有关矩阵范数的不等式：

(a)  $\|AB\|_{F} \leqslant \|A\|_{2}\|B\|_{F}$,

<a id="exercise-6-3"></a>

#### 习题 6.3

利用定理6.5证明推论6.2.

#### 对偶问题

<a id="eq-5-4-14"></a>

$$
\begin{array}{cl}\max\limits_{y}&b^{\mathrm{T}}y,\\ \text{s.t.}&A^{\mathrm{T}}y\leq c.\end{array}\tag{5.4.14}
$$

事实上，由对偶问题[(5.4.13)](#eq-5-4-13)也可以消掉变量 s 得到[(5.4.14)](#eq-5-4-14).

##### 算法6.2 非单调线搜索的 BB 方法

1. 给定  $x^0$，选取初值  $\alpha > 0$，整数  $M \geq 0$， $c_1, \beta, \varepsilon \in (0,1)$， $k = 0$。
2. $\text{while} \|\nabla f(x^k)\| > \varepsilon \text{ do}$
3. $\text{while} f(x^k - \alpha \nabla f(x^k)) \geq \max_{0 \leq j \leq \min(k, M)} f(x^{k-j}) - c_1 \alpha \|\nabla f(x^k)\|^2 \text{ do}$
4. $\alpha \leftarrow \beta \alpha$。
5. end while
6. $\text{令} x^{k+1} = x^k - \alpha \nabla f(x^k)$。
7. 根据公式 [(6.2.9)](#eq-6-2-9) 之一计算  $\alpha$，并做截断使得  $\alpha \in [\alpha_m, \alpha_M]$。
8. $k \leftarrow k + 1$。
9. end while

### 次梯度算法的收敛性

定理 6.5 (次梯度算法的收敛性) 在假设6.1的条件下，设  $\{\alpha_k > 0\}$ 为任意步长序列， $\{x^k\}$ 是由算法[(6.3.1)](#eq-6-3-1)产生的迭代序列，则对任意的  $k \geq 0$，有

$$
2\left(\sum_{i=0}^{k}\alpha_{i}\right)(\hat{f}^{k}-f^{*})\leqslant\|x^{0}-x^{*}\|^{2}+\sum_{i=0}^{k}\alpha_{i}^{2}G^{2},\tag{6.3.2}
$$

推论 6.2 在假设6.1的条件下，次梯度算法的收敛性满足（$\hat{f}^{k}$ 的定义和定理6.5中的定义相同）：
