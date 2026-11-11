# QARP scoring strategy

Midas currently implements one fundamental scoring strategy: **Quality at a
Reasonable Price (QARP)**. It is an explainable, cross-sectional research
ranking, not a forecast of returns or a recommendation to buy a security.
Eligibility and score are separate: stocks that fail eligibility checks are
reported with reasons and are not assigned a ranking score.

## What data is scored

The full QARP evaluation uses saved Fundamentus detail snapshots. The detail
view includes the current stock in its comparison universe even if it has not
yet been saved. The `--score` CLI command uses only snapshots already in the
database.

The available fields are current snapshots, not a time series:

| Category | Weight | Indicators | Direction |
|---|---:|---|---|
| Value | 30% | P/L, EV/EBITDA, P/VP | Lower is better |
| Quality | 30% | ROIC, ROE, EBIT margin, net margin | Higher is better |
| Growth | 20% | Revenue growth, earnings growth | Higher is better |
| Financial health | 15% | Net debt / equity | Lower is better |
| Dividends | 5% | Dividend yield | Higher is better |

Indicators have equal weights within each category by default. Growth data is
not available in the current Fundamentus snapshot, so growth has no usable
observations and is excluded from effective category weights. Dividend yield
is available, but dividend history and payout ratio are not; the dividend
score therefore does not measure sustainability.

The Fundamentus list uses a separate **summary-only** QARP evaluation because
its summary endpoint provides fewer fields: P/L, P/VP, ROE, and dividend yield.
Its available category weights are reweighted to sum to 100%. It does not use
EV/EBITDA, ROIC, margins, leverage, or growth. Quote dates are not available in
the summary rows, so list scoring cannot assess quote staleness.

## Eligibility

Eligibility is evaluated before scoring. By default, a stock is excluded if:

- P/L is missing, invalid, zero, or negative. Positive earnings are required
  because this strategy uses P/L as a mandatory valuation indicator.
- Overall coverage of the configured indicators in the categories active for
  the input universe is below 50%.
- Coverage within any active category is below 50%.
- Its quote date is more than 90 days before the configured evaluation date,
  or is later than that date.

The quote-date requirement is disabled by default: a missing or unparseable
date produces a warning, not an exclusion. The list summary evaluation cannot
apply the 90-day check because summary rows have no quote date.

An ineligible stock has no score and no rank. Reasons include
`NegativeEarnings`, `MissingMandatoryIndicator`, `InsufficientOverallCoverage`,
`InsufficientCategoryCoverage`, `StaleQuote`, `MissingQuoteDate`, and
`FutureQuoteDate`. Strategies can provide additional, strategy-specific
exclusion reasons.

`minimum_universe_size` defaults to 5. Falling below it adds a warning because
percentiles may not be meaningful; it does not exclude stocks. A one-stock
comparison scores valid indicators at 50.

## Normalization and score formula

For each configured indicator, only finite, valid observations from eligible
stocks enter that indicator's distribution. Missing values are omitted; they
are never replaced with zero.

The implementation sorts indicator values (or their direction-adjusted
values) and assigns a percentile using the observation's midrank:

```text
percentile = 100 * midrank / (number_of_observations - 1)
```

For ties, `midrank` is the average of the tied observations' zero-based ranks.
Higher-is-better indicators use the value directly. Lower-is-better indicators
reverse the direction before ranking. A target indicator ranks by closeness to
its configured target. Optional winsorization clamps the candidate to
configured quantiles before ranking.

Within each category, the indicator scores are multiplied by their configured
indicator weights and added. Missing indicators contribute no points; the
remaining indicator weights are **not** renormalized. Thus, missing data
cannot improve a stock's category score by making another indicator count
more.

The total score is the weighted sum of category scores using the configured
category weights. Category weights are renormalized across categories that
have at least one valid observation somewhere in the eligible scoring
universe. A category with no observations anywhere is omitted from the total.
Category scores and the total are rounded to two decimal places and kept on a
0–100 scale. The report includes both configured and effective category
weights, indicator values and scores, contributions, and coverage.

## Input handling and limitations

Fundamentus numbers are parsed using Brazilian number formatting (for example,
`1.234,56`). Missing markers, parse failures, NaN, and infinity are unavailable.
P/L, P/VP, and EV/EBITDA must be positive to be used as ordinary valuation
multiples. Net debt/equity is derived from net debt divided by positive
shareholders' equity; zero or missing equity makes it unavailable. Growth
values are not inferred from current revenue or earnings snapshots.

The source does not provide financial-statement publication dates, historical
fundamentals, dividend history, or historical universe membership. The quote
date is not a statement publication date. As a result, this implementation
does not support point-in-time backtesting and may not be suitable for
historical comparisons.

Sector-relative normalization and sector-specific indicator exclusions can be
configured, but Midas does not infer a sector taxonomy or apply built-in
sector-specific rules. No default sector adjustment is applied.

## Running and configuration

Print a JSON report from saved detail snapshots:

```powershell
cargo run -- --score
```

Select a database or provide a QARP configuration:

```powershell
cargo run -- --score --db midas.db --score-config qarp.json
```

Configuration values omitted from JSON use `QarpConfig` defaults. Category
weights must be between 0 and 1 and sum to 1 within a tolerance of `1e-8`.
Configured indicator weights in each category must also sum to 1. Coverage
thresholds and winsorization percentiles must be in `[0, 1]`, with the lower
winsorization percentile below the upper one. Mandatory indicators must exist
in the indicator configuration.

Example configuration fragment:

```json
{
  "minimum_overall_coverage": 0.6,
  "minimum_category_coverage": 0.5,
  "minimum_universe_size": 5,
  "max_quote_age_days": 90,
  "sector_relative": true,
  "winsorize_percentiles": [0.05, 0.95]
}
```

The output contains the strategy name and version, evaluation date,
configuration ID, ranked eligible stocks, separately listed ineligible
stocks, category and indicator scores, coverage, weights, warnings, and
exclusion reasons.

## Adding another strategy

The scoring engine accepts `StockFundamentals` directly, so strategy tests do
not need live scraping. Implement `ScoringStrategy` to provide a name, version,
validated configuration, and any extra eligibility checks. Pass the strategy
and snapshot universe to `run_strategy`. Deep Value, Dividend Income, GARP,
Quality, and Piotroski-inspired strategies are not implemented yet.
