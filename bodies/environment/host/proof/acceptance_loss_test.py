"""The relay may inject one lost acceptance, never a repeated outage."""
import unittest
from acceptance_loss import SingleAcceptanceLoss

class SingleLossTest(unittest.TestCase):
    def test_only_one_actual_acceptance_is_selected(self):
        loss = SingleAcceptanceLoss()
        accepted = {"event": "native-display-check-control", "acknowledgement": {"state": "EvidenceAccepted"}}
        self.assertFalse(loss.should_drop(accepted))
        loss.armed = True
        for other in [None, 1, [], {"event": "other"}, {"event": "native-display-check-control", "acknowledgement": None}, {"event": "native-display-check-control", "acknowledgement": 1}]:
            self.assertFalse(loss.should_drop(other))
        self.assertTrue(loss.should_drop(accepted))
        self.assertFalse(loss.armed)
        for _ in range(10):
            self.assertFalse(loss.should_drop(accepted))
        loss.armed = True
        self.assertFalse(loss.should_drop(accepted))

if __name__ == "__main__":
    unittest.main()
