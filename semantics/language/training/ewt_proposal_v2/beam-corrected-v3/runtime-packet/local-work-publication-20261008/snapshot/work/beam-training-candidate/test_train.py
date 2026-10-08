"""Focused training-loss regression; no runtime acceptance authority."""
import collections,json,pathlib,struct,unittest
import train
class LossTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.profile={e['surface']:e['pos_codes']for f in sorted(pathlib.Path('/home/dancxjo/conduit-4907-evaluation-identity/work/lexical-proposer-next/train-dictionary-authored-overlay-v2').glob('shard-*.json'))for e in json.loads(f.read_text())['entries']}
  cls.rows=json.load(open('work/corrected411-runtime/diagnostic-rows.json'))
  cls.weights=struct.unpack('<31236h',(train.BASE/'proposal_window8.i16').read_bytes()[20:])
 def test_actual_diagnosed_loss_locations_and_structured_margin(self):
  for row,expected in zip(self.rows,[('early',3),('final',14),('final',8),('final',14)]):
   positive,negative,kind,epoch=train.search(row,self.profile,self.weights)
   self.assertEqual((kind,epoch),expected)
   difference=collections.Counter(positive);difference.subtract(negative)
   delta=sum(256*n*n for n in difference.values())
   self.assertGreater(delta,0)
   updated=list(self.weights)
   for i,n in difference.items():updated[i]+=256*n
   old=sum(self.weights[i]*n for i,n in difference.items())
   new=sum(updated[i]*n for i,n in difference.items())
   self.assertEqual(new-old,delta)
 def test_gold_outside_candidates_refuses(self):
  row=self.rows[0];profile=dict(self.profile);profile[row['forms'][0]]=[]
  with self.assertRaisesRegex(ValueError,'empty legal beam'):train.search(row,profile,self.weights)
if __name__=='__main__':unittest.main()
