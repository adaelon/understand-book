# 期末复习题 {#review-questions}

以下 14 道题供独立练习，解答见配套解答册的同名部分。例 3、例 4 与前面章节的题目相同，可以在复习时再做一遍。

<!-- question id="review-01" kind="review" source="review" pages="10" -->
## 例 1　凸函数的充要条件 {#review01}

写出函数 $f:\mathbb R^n\to\mathbb R$ 为凸函数的至少三个充要条件。

*对应阅读：第 3.4 节。*
<!-- endquestion -->

<!-- question id="review-02" kind="review" source="review" pages="11" -->
## 例 2　凸函数的水平集 {#review02}

证明凸函数 $f$ 的水平集

$$L(x_0)=\{x\in\mathbb R^n:f(x)\leq f(x_0)\}$$

为凸集。

*对应阅读：第 3.4 节。*
<!-- endquestion -->

<!-- question id="review-03" kind="review" source="review" pages="12,13" -->
## 例 3　上图判别法 {#review03}

试证明函数 $f$ 为凸函数，当且仅当

$$\operatorname{epi}f=\{(x,\alpha):f(x)\leq\alpha\}$$

为凸集。

*与第 3 章作业题 3 相同。*
<!-- endquestion -->

<!-- question id="review-04" kind="review" source="review" pages="21" -->
## 例 4　由对偶解求原问题解 {#review04}

考虑下面一对线性规划问题：

$$\begin{aligned}
(P)\quad\max\quad&x_1+2x_2+3x_3+4x_4,\\
\text{s.t.}\quad&x_1+2x_2+2x_3+3x_4\leq20,\\
&2x_1+x_2+3x_3+2x_4\leq20,\\
&x_1,x_2,x_3,x_4\geq0;
\end{aligned}$$

$$\begin{aligned}
(D)\quad\min\quad&20y_1+20y_2,\\
\text{s.t.}\quad&y_1+2y_2\geq1,\quad2y_1+y_2\geq2,\\
&2y_1+3y_2\geq3,\quad3y_1+2y_2\geq4,\\
&y_1,y_2\geq0.
\end{aligned}$$

已知 $(D)$ 的最优解为 $y^*=(6/5,1/5)^T$，用互补松弛定理求出 $(P)$ 的最优解。

*第 17.4 节已讲解同一例题。*
<!-- endquestion -->

<!-- question id="review-05" kind="review" source="review" pages="23" -->
## 例 5　由最优解确定参数 {#review05}

设 $b$ 是一个常数，考虑线性规划问题

$$\begin{aligned}
\min\quad&5x_1+14x_3,\\
\text{s.t.}\quad&x_1-x_2+3x_3\geq b,\\
&x_1+x_2+2x_3\geq4,\qquad x_1,x_2,x_3\geq0.
\end{aligned}$$

如果 $x^*=(2,0,1)^T$ 是该问题的最优解，则 $b$ 为多少？其对偶问题的最优解是什么？

*对应阅读：第 17.4 节。*
<!-- endquestion -->

<!-- question id="review-06" kind="review" source="review" pages="25" -->
## 例 6　由对偶证明最优值 {#review06}

设 $A\in\mathbb R^{m\times n}$、$b\in\mathbb R^m$、$c\in\mathbb R^n$，考虑线性规划问题

$$\begin{aligned}
(P)\quad\min\quad&c^Tx-b^Ty,\\
\text{s.t.}\quad&Ax\geq b,\qquad A^Ty\leq c,\\
&x\geq0,\qquad y\geq0.
\end{aligned}$$

（1）写出其对偶规划。

（2）试证明 $(P)$ 要么不可行，要么有最优解且最优值为 $0$。

*对应阅读：第 17.1、17.3 节。*
<!-- endquestion -->

<!-- question id="review-07" kind="review" source="review" pages="34" -->
## 例 7　两阶段法与对偶问题 {#review07}

用两阶段法求解标准线性规划 $\min c^Tx$，约束为 $Ax=b,x\geq0$。已知 $c=(2,3,2,2)^T$，且第一阶段的初始单纯形表（表 I）和最优单纯形表（表 II）如下。$y_1,y_2$ 为人工变量。

$$\begin{array}{c|rrrrrr|r}
\text{表 I}&x_1&x_2&x_3&x_4&y_1&y_2&\text{右端}\\\hline
y_1&1&2&1&3&1&0&\square\\
y_2&1&1&2&4&0&1&\square\\\hline
q&2&3&3&7&0&0&8
\end{array}$$

$$\begin{array}{c|rrrrrr|r}
\text{表 II}&x_1&x_2&x_3&x_4&y_1&y_2&\text{右端}\\\hline
x_4&1/2&3/2&0&1&1&-1/2&1/2\\
x_3&-1/2&-5/2&1&0&-2&3/2&3/2\\\hline
q&0&0&0&0&-1&-1&0
\end{array}$$

（1）确定所给的线性规划问题，并求该问题的最优解。

（2）写出该问题的对偶问题，并求对偶问题的最优解。

*第 18.6 节讲解过表中的基解；本题另要求求对偶解。*
<!-- endquestion -->

<!-- question id="review-08" kind="review" source="review" pages="36" -->
## 例 8　补全最优单纯形表 {#review08}

考虑线性规划问题

$$\begin{aligned}
(P2)\quad\max\quad&2x_1-x_2+x_3,\\
\text{s.t.}\quad&3x_1+x_2+x_3\leq b_1,\\
&x_1-x_2+2x_3\leq b_2,\\
&x_1+x_2-x_3\leq b_3,\qquad x_1,x_2,x_3\geq0.
\end{aligned}$$

利用单纯形法求解后得到如下最优单纯形表。$x_4,x_5,x_6$ 为三条约束依次引入的松弛变量，$\square$ 表示待填位置。

$$\begin{array}{c|rrrrrr|r}
\text{基}&x_1&x_2&x_3&x_4&x_5&x_6&\text{右端}\\\hline
&\square&\square&\square&\square&\square&\square&\square\\\hline
x_4&\square&\square&\square&\square&-1&-2&10\\
x_1&\square&\square&\square&\square&1/2&1/2&15\\
x_2&\square&\square&\square&\square&-1/2&1/2&5
\end{array}$$

（1）确定 $b_1,b_2,b_3$ 的值，并把最优表补充完整。

（2）写出 $(P2)$ 的对偶问题，并根据给出的最优表求其对偶问题的最优解。

*对应阅读：第 18.2—18.3、18.8 节。*
<!-- endquestion -->

<!-- question id="review-09" kind="review" source="review" pages="46" -->
## 例 9　Lagrange 对偶与共轭关系 {#review09}

设 $Q\in\mathbb R^{n\times n}$ 对称正定，$b\in\mathbb R^n$ 且 $b\ne0$，考虑非线性规划问题

$$\min\ \frac12x^TQx,\qquad x\geq b.$$

（1）写出该问题的 Lagrange 对偶规划。

（2）设 $x^*$ 是原问题的最优解，证明 $x^*$ 与 $x^*-b$ 关于 $Q$ 共轭。

*对应阅读：第 9.1、13.1、16.1—16.3 节。*
<!-- endquestion -->

<!-- question id="review-10" kind="review" source="review" pages="48" -->
## 例 10　一阶必要条件与二阶充分条件 {#review10}

考虑约束优化问题

$$\min\ x_1x_2,\qquad 2x_1-x_2-3=0.$$

给定 $\bar x=(3/4,-3/2)^T$，利用约束优化问题局部解的一阶必要条件和二阶充分条件，判断 $\bar x$ 是否为原问题的局部最优解。

*对应阅读：第 13、14 章。*
<!-- endquestion -->

<!-- question id="review-11" kind="review" source="review" pages="52" -->
## 例 11　无约束极值计算 {#review11}

设 $p,q>1$，且满足 $1/p+1/q=1$。定义

$$f(y)=\max_{x\in\mathbb R^n}
\left\{x^Ty-\frac1p\|x\|_2^p\right\}.$$

证明 $f(y)=\|y\|_2^q/q$。

*对应阅读：第 2、4 章。*
<!-- endquestion -->

<!-- question id="review-12" kind="review" source="review" pages="53" -->
## 例 12　最速下降法迭代两步 {#review12}

取初始点 $x^{(0)}=(1,1)^T$，采用精确线搜索的最速下降法求解下面的无约束优化问题，迭代两步：

$$\min f(x)=x_1^2+2x_2^2.$$

*对应阅读：第 6、7 章。*
<!-- endquestion -->

<!-- question id="review-13" kind="review" source="review" pages="57" -->
## 例 13　FR 共轭梯度法的下一步 {#review13}

将 FR 共轭梯度法用于有三个变量的函数 $f(x)$。第一次迭代的搜索方向为 $d^{(1)}=(2,-1,1)^T$，沿 $d^{(1)}$ 作精确一维搜索，得到新的迭代点 $x$，且

$$\nabla f(x)=(-2,-3,\beta)^T.$$

求 $\beta$。按 FR 共轭梯度法继续，从迭代点 $x$ 出发的搜索方向是什么？

*对应阅读：第 6.1、9.2、9.5 节。*
<!-- endquestion -->

<!-- question id="review-14" kind="review" source="review" pages="59" -->
## 例 14　外点罚函数法 {#review14}

利用外点罚函数法求解约束优化问题，取罚参数为 $\mu$：

$$\min\ x_1^2+x_2^2,\qquad x_1+x_2=1.$$

*对应阅读：第 19.2 节。*
<!-- endquestion -->
