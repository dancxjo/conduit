/* Development-only pinned scalar conditioning oracle, no product owner. */
#include <stdio.h>
#include <string.h>
#include "fargan.c"
int main(int argc,char **argv) {
  FARGANState st; FILE *in,*out; float features[20],cond[320];int period, previous;
  if(argc!=3)return 2;
  in=fopen(argv[1],"rb");out=fopen(argv[2],"wb");if(!in||!out)return 3;
  fargan_init(&st);st.last_period=69;
  while(fread(features,4,20,in)==20) {
    previous=st.last_period;period=(int)floor(.5+256./pow(2.f,features[18]+1.5));
    compute_fargan_cond(&st,cond,features,period);
    fwrite(cond,4,320,out);fwrite(st.cond_conv1_state,4,128,out);
    fwrite(&previous,4,1,out);fwrite(&period,4,1,out);st.last_period=period;
  }
  fclose(in);fclose(out);return 0;
}
