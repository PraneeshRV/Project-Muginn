from eval.recall_eval import evaluate


def test_selector_meets_quality_bar():
    m = evaluate()
    assert m["recall"] >= 0.8          # catch most salient facts
    assert m["false_positive_rate"] <= 0.25  # leak few non-facts
