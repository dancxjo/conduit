"""Focused training-loss regression; no runtime acceptance authority."""
import ast,collections,json,pathlib,struct,unittest
import train
class LossTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.profile={e['surface']:e['pos_codes']for f in sorted(pathlib.Path(str(pathlib.Path(__file__).resolve().parents[2]/'ewt_proposal_v1/train-dictionary-authored-overlay-v2')).glob('shard-*.json'))for e in json.loads(f.read_text())['entries']}
  cls.rows=json.loads((pathlib.Path(__file__).resolve().parent/'teaching-regression-rows.json').read_text())
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
 def test_original_feature_function_asts_unchanged(self):
  original=ast.parse((train.BASE/'reference_v2_corrected.py').read_text())
  current=ast.parse(pathlib.Path(train.ref.__file__).read_text())
  functions=lambda tree:{node.name:ast.dump(node,include_attributes=False)for node in tree.body if isinstance(node,ast.FunctionDef)}
  self.assertEqual(functions(original),functions(current))
 def test_gold_outside_candidates_refuses(self):
  row=self.rows[0];profile=dict(self.profile);profile[row['forms'][0]]=[]
  with self.assertRaisesRegex(ValueError,'empty legal beam'):train.search(row,profile,self.weights)
if __name__=='__main__':unittest.main()
