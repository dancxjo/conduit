/* Development-only pinned scalar float oracle; no product linkage. */
#include <stdio.h>
#include "fargan.c"
_Static_assert(COND_NET_FCONV1_STATE_SIZE == 128, "conditioning state extent");
_Static_assert(SIG_NET_INPUT_SIZE == 164, "signal convolution active history extent");
int main(int argc,char **argv) {
  FARGANState st;
  float features[100], initial[320]={0};
  FILE *in,*out;
  if(argc!=3)return 2;
  in=fopen(argv[1],"rb");if(!in)return 3;
  if(fread(features,sizeof(float),100,in)!=100 || fgetc(in)!=EOF)return 4;
  fclose(in);fargan_init(&st);if(st.model.sig_net_fwc0_conv.nb_inputs != 328)return 6;fargan_cont(&st,initial,features);
  out=fopen(argv[2],"wb");if(!out)return 5;
  fwrite(st.cond_conv1_state,4,128,out);
  fwrite(st.fwc0_mem,4,164,out);fwrite(st.gru1_state,4,160,out);
  fwrite(st.gru2_state,4,128,out);fwrite(st.gru3_state,4,128,out);
  fwrite(st.pitch_buf,4,256,out);fwrite(&st.deemph_mem,4,1,out);
  fwrite(&st.last_period,4,1,out);fclose(out);return 0;
}
